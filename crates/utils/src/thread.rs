// Thanks to Rust-Analyzer!

use std::fmt;

use crossbeam_channel::{Receiver, Sender};

pub struct Pool {
    // `_handles` is never read: the field is present only for its `Drop` impl.
    // The worker threads exit once the channel closes;
    //
    // <ake sure to keep `job_sender` above `handles` so that the channel is
    // actually closed before we join the worker threads!
    job_sender: Sender<Job>,
    _handles: Vec<JoinHandle>,
}

struct Job {
    requested_intent: ThreadIntent,
    f: Box<dyn FnOnce() + Send + 'static>,
}

impl Pool {
    pub fn new(threads: usize) -> Pool {
        const STACK_SIZE: usize = 8 * 1024 * 1024;
        const INITIAL_INTENT: ThreadIntent = ThreadIntent::Worker;

        let (job_sender, job_receiver) = crossbeam_channel::unbounded();

        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            let handle = match Builder::new(INITIAL_INTENT)
                .stack_size(STACK_SIZE)
                .name("Worker".into())
                .spawn({
                    let job_receiver: Receiver<Job> = job_receiver.clone();
                    move || {
                        let mut current_intent = INITIAL_INTENT;
                        for job in job_receiver {
                            if job.requested_intent != current_intent {
                                job.requested_intent.apply_to_current_thread();
                                current_intent = job.requested_intent;
                            }
                            (job.f)();
                        }
                    }
                }) {
                Ok(handle) => handle,
                Err(err) => {
                    tracing::error!(%err, "failed to spawn worker thread");
                    continue;
                }
            };

            handles.push(handle);
        }

        Pool { _handles: handles, job_sender }
    }

    pub fn spawn<F>(&self, intent: ThreadIntent, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let f = Box::new(move || {
            if cfg!(debug_assertions) {
                intent.assert_is_used_on_current_thread();
            }
            f()
        });

        let job = Job { requested_intent: intent, f };
        if let Err(err) = self.job_sender.send(job) {
            let job = err.into_inner();
            tracing::debug!("worker pool is closed; executing job inline");
            (job.f)();
        }
    }
}

#[allow(clippy::expect_used)]
pub fn spawn<F, T>(intent: ThreadIntent, f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    Builder::new(intent).spawn(f).expect("failed to spawn thread")
}

pub struct Builder {
    intent: ThreadIntent,
    inner: jod_thread::Builder,
    allow_leak: bool,
}

impl Builder {
    pub fn new(intent: ThreadIntent) -> Builder {
        Builder { intent, inner: jod_thread::Builder::new(), allow_leak: false }
    }

    pub fn name(self, name: String) -> Builder {
        Builder { inner: self.inner.name(name), ..self }
    }

    pub fn stack_size(self, size: usize) -> Builder {
        Builder { inner: self.inner.stack_size(size), ..self }
    }

    pub fn allow_leak(self, b: bool) -> Builder {
        Builder { allow_leak: b, ..self }
    }

    pub fn spawn<F, T>(self, f: F) -> std::io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let inner_handle = self.inner.spawn(move || {
            self.intent.apply_to_current_thread();
            f()
        })?;

        Ok(JoinHandle { inner: Some(inner_handle), allow_leak: self.allow_leak })
    }
}

pub struct JoinHandle<T = ()> {
    inner: Option<jod_thread::JoinHandle<T>>,
    allow_leak: bool,
}

impl<T> JoinHandle<T> {
    #[allow(clippy::unwrap_used)]
    pub fn join(mut self) -> T {
        self.inner.take().unwrap().join()
    }
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        if !self.allow_leak {
            return;
        }

        if let Some(join_handle) = self.inner.take() {
            join_handle.detach();
        }
    }
}

impl<T> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("JoinHandle { .. }")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// Please maintain order from least to most priority for the derived `Ord` impl.
pub enum ThreadIntent {
    Worker,
    LatencySensitive,
}

impl ThreadIntent {
    // These APIs must remain private. We only want consumers to set thread intent
    // either during thread creation or using our pool impl.

    pub(super) fn apply_to_current_thread(self) {
        let class = thread_intent_to_qos_class(self);
        set_current_thread_qos_class(class);
    }

    pub(super) fn assert_is_used_on_current_thread(self) {
        if IS_QOS_AVAILABLE {
            let class = thread_intent_to_qos_class(self);
            assert_eq!(get_current_thread_qos_class(), Some(class));
        }
    }
}

use imp::QoSClass;

const IS_QOS_AVAILABLE: bool = imp::IS_QOS_AVAILABLE;

fn set_current_thread_qos_class(class: QoSClass) {
    imp::set_current_thread_qos_class(class)
}

fn get_current_thread_qos_class() -> Option<QoSClass> {
    imp::get_current_thread_qos_class()
}

fn thread_intent_to_qos_class(intent: ThreadIntent) -> QoSClass {
    imp::thread_intent_to_qos_class(intent)
}

// All Apple platforms use XNU as their kernel
// and thus have the concept of QoS.
#[cfg(target_vendor = "apple")]
mod imp {
    use super::ThreadIntent;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    // Please maintain order from least to most priority for the derived `Ord` impl.
    pub(super) enum QoSClass {
        // Documentation adapted from https://github.com/apple-oss-distributions/libpthread/blob/67e155c94093be9a204b69637d198eceff2c7c46/include/sys/qos.h#L55
        /// TLDR: invisible maintenance tasks
        ///
        /// Contract:
        ///
        /// * **You do not care about how long it takes for work to finish.**
        /// * **You do not care about work being deferred temporarily.** (e.g.
        ///   if the device’s battery is in a critical state)
        ///
        /// Use this QoS class for background tasks which the user did not
        /// initiate themselves and which are invisible to the user.
        /// It is expected that this work will take significant time to
        /// complete: minutes or even hours.
        ///
        /// This QoS class provides the most energy and thermally-efficient
        /// execution possible. All other work is prioritized over
        /// background tasks.
        Background,

        /// TLDR: tasks that don’t block using your app
        ///
        /// Contract:
        ///
        /// * **Your app remains useful even as the task is executing.**
        ///
        /// Use this QoS class for tasks which may or may not be initiated by
        /// the user, but whose result is visible.
        /// It is expected that this work will take a few seconds to a few
        /// minutes. Typically your app will include a progress bar
        /// for tasks using this class.
        ///
        /// This QoS class provides a balance between
        /// performance, responsiveness and efficiency.
        Utility,

        /// TLDR: tasks that block using your app
        ///
        /// Contract:
        ///
        /// * **You need this work to complete before the user can keep
        ///   interacting with your app.**
        /// * **Your work will not take more than a few seconds to complete.**
        ///
        /// Use this QoS class for tasks which were initiated by the user
        /// and block the usage of your app while they are in progress.
        /// It is expected that this work will take a few seconds or less to
        /// complete; not long enough to cause the user to switch to
        /// something else. Your app will likely indicate progress on
        /// these tasks through the display of placeholder content or
        /// modals.
        ///
        /// This QoS class is not energy-efficient.
        /// Rather, it provides responsiveness
        /// by prioritizing work above other tasks on the system
        /// except for critical user-interactive work.
        UserInitiated,

        /// TLDR: render loops and nothing else
        ///
        /// Contract:
        ///
        /// * **You absolutely need this work to complete immediately or your
        ///   app will appear to freeze.**
        /// * **Your work will always complete virtually instantaneously.**
        ///
        /// Use this QoS class for any work which, if delayed,
        /// will make your user interface unresponsive.
        /// It is expected that this work will be virtually instantaneous.
        ///
        /// This QoS class is not energy-efficient.
        /// Specifying this class is a request to run with
        /// nearly all available system CPU and I/O bandwidth even under
        /// contention.
        UserInteractive,
    }

    pub(super) const IS_QOS_AVAILABLE: bool = true;

    pub(super) fn set_current_thread_qos_class(class: QoSClass) {
        let c = match class {
            QoSClass::UserInteractive => libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE,
            QoSClass::UserInitiated => libc::qos_class_t::QOS_CLASS_USER_INITIATED,
            QoSClass::Utility => libc::qos_class_t::QOS_CLASS_UTILITY,
            QoSClass::Background => libc::qos_class_t::QOS_CLASS_BACKGROUND,
        };

        let code = unsafe { libc::pthread_set_qos_class_self_np(c, 0) };

        if code == 0 {
            return;
        }

        let errno = unsafe { *libc::__error() };

        match errno {
            libc::EPERM => {
                // This thread has been excluded from the QoS system
                // due to a previous call to a function such as `pthread_setschedparam`
                // which is incompatible with QoS.
                //
                // Panic instead of returning an error
                // to maintain the invariant that we only use QoS APIs.
                panic!("tried to set QoS of thread which has opted out of QoS (os error {errno})")
            }

            libc::EINVAL => {
                // This is returned if we pass something other than a qos_class_t
                // to `pthread_set_qos_class_self_np`.
                //
                // This is impossible, so again panic.
                unreachable!(
                    "invalid qos_class_t value was passed to pthread_set_qos_class_self_np"
                )
            }

            _ => {
                // `pthread_set_qos_class_self_np`’s documentation
                // does not mention any other errors.
                unreachable!("`pthread_set_qos_class_self_np` returned unexpected error {errno}")
            }
        }
    }

    pub(super) fn get_current_thread_qos_class() -> Option<QoSClass> {
        let current_thread = unsafe { libc::pthread_self() };
        let mut qos_class_raw = libc::qos_class_t::QOS_CLASS_UNSPECIFIED;
        let code = unsafe {
            libc::pthread_get_qos_class_np(current_thread, &mut qos_class_raw, std::ptr::null_mut())
        };

        if code != 0 {
            // `pthread_get_qos_class_np`’s documentation states that
            // an error value is placed into errno if the return code is not zero.
            // However, it never states what errors are possible.
            // Inspecting the source[0] shows that, as of this writing, it always returns
            // zero.
            //
            // Whatever errors the function could report in future are likely to be
            // ones which we cannot handle anyway
            //
            // 0: https://github.com/apple-oss-distributions/libpthread/blob/67e155c94093be9a204b69637d198eceff2c7c46/src/qos.c#L171-L177
            let errno = unsafe { *libc::__error() };
            unreachable!("`pthread_get_qos_class_np` failed unexpectedly (os error {errno})");
        }

        match qos_class_raw {
            libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE => Some(QoSClass::UserInteractive),
            libc::qos_class_t::QOS_CLASS_USER_INITIATED => Some(QoSClass::UserInitiated),
            libc::qos_class_t::QOS_CLASS_DEFAULT => None, // QoS has never been set
            libc::qos_class_t::QOS_CLASS_UTILITY => Some(QoSClass::Utility),
            libc::qos_class_t::QOS_CLASS_BACKGROUND => Some(QoSClass::Background),

            libc::qos_class_t::QOS_CLASS_UNSPECIFIED => {
                // Using manual scheduling APIs causes threads to “opt out” of QoS.
                // At this point they become incompatible with QoS,
                // and as such have the “unspecified” QoS class.
                //
                // Panic instead of returning an error
                // to maintain the invariant that we only use QoS APIs.
                panic!("tried to get QoS of thread which has opted out of QoS")
            }
        }
    }

    pub(super) fn thread_intent_to_qos_class(intent: ThreadIntent) -> QoSClass {
        match intent {
            ThreadIntent::Worker => QoSClass::Utility,
            ThreadIntent::LatencySensitive => QoSClass::UserInitiated,
        }
    }
}

// TODO: QoS for Windows
#[cfg(not(target_vendor = "apple"))]
mod imp {
    use super::ThreadIntent;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(super) enum QoSClass {
        Default,
    }

    pub(super) const IS_QOS_AVAILABLE: bool = false;

    pub(super) fn set_current_thread_qos_class(_: QoSClass) {}

    pub(super) fn get_current_thread_qos_class() -> Option<QoSClass> {
        None
    }

    pub(super) fn thread_intent_to_qos_class(_: ThreadIntent) -> QoSClass {
        QoSClass::Default
    }
}
