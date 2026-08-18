use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Condvar, Mutex};
use triomphe::Arc;

/// Who is asking for a product.
///
/// A [`Foreground`](ComputationPriority::Foreground) request must not wait for
/// a slower [`Background`](ComputationPriority::Background) prewarm, so it
/// supersedes an in-flight background computation. Two foreground callers
/// share one computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ComputationPriority {
    Background,
    Foreground,
}

/// One in-flight computation, tagged with the generation that started it so a
/// superseded computation can discard its result instead of publishing.
struct InFlight {
    generation: u64,
    priority: ComputationPriority,
    cancel: std::sync::Arc<AtomicBool>,
}

struct ProductState<T> {
    generation: u64,
    value: Option<Arc<T>>,
    in_flight: Option<InFlight>,
}

impl<T> Default for ProductState<T> {
    fn default() -> Self {
        Self { generation: 0, value: None, in_flight: None }
    }
}

/// A memoized structure product computed once and reused across concurrent
/// requests.
///
/// Generation model: every computation bumps a generation counter. The result
/// of a computation is published only while its generation is still current;
/// a foreground request that supersedes a background prewarm starts a newer
/// generation, and the background's late result is discarded. The mutex guards
/// state transitions only; `compute` always runs outside it.
pub(crate) struct ProductCell<T> {
    state: Mutex<ProductState<T>>,
    ready: Condvar,
}

impl<T> Default for ProductCell<T> {
    fn default() -> Self {
        Self { state: Mutex::new(ProductState::default()), ready: Condvar::new() }
    }
}

impl<T> ProductCell<T> {
    pub(crate) fn peek(&self) -> Option<Arc<T>> {
        self.state.lock().value.clone()
    }

    pub(crate) fn from_arc(value: Arc<T>) -> Self {
        Self {
            state: Mutex::new(ProductState { generation: 0, value: Some(value), in_flight: None }),
            ready: Condvar::new(),
        }
    }

    pub(crate) fn get_or_compute_foreground(
        &self,
        compute: impl FnOnce(&AtomicBool) -> Option<Arc<T>>,
    ) -> Arc<T> {
        self.get_or_compute(ComputationPriority::Foreground, &AtomicBool::new(false), compute)
            .unwrap_or_else(|| unreachable!("foreground product has no cancel token"))
    }

    pub(crate) fn get_or_compute(
        &self,
        priority: ComputationPriority,
        external_cancel: &AtomicBool,
        compute: impl FnOnce(&AtomicBool) -> Option<Arc<T>>,
    ) -> Option<Arc<T>> {
        let mut compute = Some(compute);
        loop {
            let (generation, cancel) = {
                let mut state = self.state.lock();
                if let Some(value) = &state.value {
                    return Some(value.clone());
                }
                if external_cancel.load(Ordering::Acquire) {
                    return None;
                }
                match &state.in_flight {
                    None => {}
                    Some(current) if priority > current.priority => {
                        current.cancel.store(true, Ordering::Release);
                    }
                    Some(_) => {
                        self.ready.wait(&mut state);
                        continue;
                    }
                }
                state.generation += 1;
                let generation = state.generation;
                let cancel = std::sync::Arc::new(AtomicBool::new(false));
                state.in_flight = Some(InFlight { generation, priority, cancel: cancel.clone() });
                (generation, cancel)
            };

            let value = compute.take().expect("a product caller computes at most once")(&cancel);
            let mut state = self.state.lock();
            let owns_slot =
                state.in_flight.as_ref().is_some_and(|current| current.generation == generation);
            if owns_slot {
                state.in_flight = None;
                let publish = value.as_ref().is_some()
                    && !cancel.load(Ordering::Acquire)
                    && !external_cancel.load(Ordering::Acquire);
                if publish {
                    state.value = value.clone();
                }
                self.ready.notify_all();
                return value.filter(|_| !external_cancel.load(Ordering::Acquire));
            }
            // A foreground request superseded this computation; its result is
            // intentionally discarded.
            self.ready.notify_all();
            if external_cancel.load(Ordering::Acquire) {
                return None;
            }
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc as StdArc, mpsc};

    use super::*;

    #[test]
    fn foreground_takes_over_background_product() {
        let cell = StdArc::new(ProductCell::<u32>::default());
        let (started_tx, started_rx) = mpsc::channel();
        let background_cell = cell.clone();
        let background = std::thread::spawn(move || {
            background_cell.get_or_compute(
                ComputationPriority::Background,
                &AtomicBool::new(false),
                |cancel| {
                    started_tx.send(()).unwrap();
                    while !cancel.load(Ordering::Acquire) {
                        std::thread::yield_now();
                    }
                    Some(Arc::new(1))
                },
            )
        });
        started_rx.recv().unwrap();

        let foreground = cell
            .get_or_compute(ComputationPriority::Foreground, &AtomicBool::new(false), |_| {
                Some(Arc::new(2))
            })
            .unwrap();

        assert_eq!(*foreground, 2);
        assert!(background.join().unwrap().is_none());
        assert_eq!(
            *cell
                .get_or_compute(ComputationPriority::Foreground, &AtomicBool::new(false), |_| {
                    Some(Arc::new(3))
                },)
                .unwrap(),
            2
        );
    }
}
