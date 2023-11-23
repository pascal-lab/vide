use std::fmt;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crossbeam_channel::{Receiver, Sender};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// Please maintain order from least to most priority for the derived `Ord` impl.
// TODO: QoS
pub enum ThreadIntent {
    Worker,
    LatencySensitive,
}

pub struct Pool {
    // `_handles` is never read: the field is present
    // only for its `Drop` impl.
    // The worker threads exit once the channel closes;
    // make sure to keep `job_sender` above `handles`
    // so that the channel is actually closed
    // before we join the worker threads!
    job_sender: Sender<Job>,
    _handles: Vec<JoinHandle>,
    extant_tasks: Arc<AtomicUsize>,
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
        let extant_tasks = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            let handle = Builder::new(INITIAL_INTENT)
                .stack_size(STACK_SIZE)
                .name("Worker".into())
                .spawn({
                    let extant_tasks = Arc::clone(&extant_tasks);
                    let job_receiver: Receiver<Job> = job_receiver.clone();
                    move || {
                        let mut current_intent = INITIAL_INTENT;
                        for job in job_receiver {
                            if job.requested_intent != current_intent {
                                current_intent = job.requested_intent;
                            }
                            extant_tasks.fetch_add(1, Ordering::SeqCst);
                            (job.f)();
                            extant_tasks.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                })
                .expect("failed to spawn thread");

            handles.push(handle);
        }

        Pool { _handles: handles, extant_tasks, job_sender }
    }

    pub fn spawn<F>(&self, intent: ThreadIntent, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let f = Box::new(move || f());

        let job = Job { requested_intent: intent, f };
        self.job_sender.send(job).unwrap();
    }

    pub fn len(&self) -> usize {
        self.extant_tasks.load(Ordering::SeqCst)
    }
}

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
        let inner_handle = self.inner.spawn(move || f())?;

        Ok(JoinHandle { inner: Some(inner_handle), allow_leak: self.allow_leak })
    }
}

pub struct JoinHandle<T = ()> {
    inner: Option<jod_thread::JoinHandle<T>>,
    allow_leak: bool,
}

impl<T> JoinHandle<T> {
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
