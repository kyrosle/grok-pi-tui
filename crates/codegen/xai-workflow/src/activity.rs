//! Shared active-work lifetime counter; no agent/session actor dependency.
pub struct WorkGuard(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl WorkGuard {
    pub fn new(active_work: std::sync::Arc<std::sync::atomic::AtomicUsize>) -> Self {
        active_work.fetch_add(1, std::sync::atomic::Ordering::Release);
        Self(active_work)
    }
}
impl Drop for WorkGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Release);
    }
}
