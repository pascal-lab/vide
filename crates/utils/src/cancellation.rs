use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug)]
pub struct CancellationToken {
    inner: Arc<CancellationState>,
}

#[derive(Debug)]
struct CancellationState {
    cancelled: AtomicBool,
    children: Mutex<Vec<Weak<CancellationState>>>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancellationState {
                cancelled: AtomicBool::new(false),
                children: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn child_token(&self) -> Self {
        let child = Self::new();
        if self.is_cancelled() {
            child.cancel();
            return child;
        }

        let mut children = self.children();
        children.retain(|child| child.strong_count() > 0);
        children.push(Arc::downgrade(&child.inner));
        drop(children);
        if self.is_cancelled() {
            child.cancel();
        }
        child
    }

    pub fn cancel(&self) {
        if self.inner.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }

        let children = {
            let mut children = self.children();
            children.retain(|child| child.strong_count() > 0);
            children.iter().filter_map(Weak::upgrade).collect::<Vec<_>>()
        };

        for child in children {
            Self { inner: child }.cancel();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    pub fn check(&self) -> Result<(), CancellationError> {
        if self.is_cancelled() { Err(CancellationError) } else { Ok(()) }
    }

    fn children(&self) -> std::sync::MutexGuard<'_, Vec<Weak<CancellationState>>> {
        self.inner.children.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    #[cfg(test)]
    fn child_slot_count(&self) -> usize {
        self.children().len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancellationError;

impl fmt::Display for CancellationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation cancelled")
    }
}

impl Error for CancellationError {}

#[cfg(test)]
mod tests {
    use super::CancellationToken;

    #[test]
    fn parent_cancellation_reaches_child_tokens() {
        let parent = CancellationToken::new();
        let child = parent.child_token();

        parent.cancel();

        assert!(child.is_cancelled());
    }

    #[test]
    fn child_created_after_parent_cancellation_is_cancelled() {
        let parent = CancellationToken::new();
        parent.cancel();

        assert!(parent.child_token().is_cancelled());
    }

    #[test]
    fn child_creation_removes_dropped_child_slots() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        assert_eq!(parent.child_slot_count(), 1);

        drop(child);
        let _next = parent.child_token();

        assert_eq!(parent.child_slot_count(), 1);
    }
}
