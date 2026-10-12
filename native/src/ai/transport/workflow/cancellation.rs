//! Observe consumer cancellation only while the replay future is suspended.
//! Returning an ordinary decode/delivery error must not satisfy cancellation.
use std::sync::atomic::{AtomicBool, Ordering};
pub(super) struct Window<'a> {
    observed: &'a AtomicBool,
    armed: bool,
}
impl<'a> Window<'a> {
    pub(super) fn new(observed: &'a AtomicBool, armed: bool) -> Self {
        Self { observed, armed }
    }
    pub(super) fn complete(&mut self) {
        self.armed = false;
    }
}
impl Drop for Window<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.observed.store(true, Ordering::Relaxed);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn consumer_cancellation_is_distinct_from_normal_completion() {
        let observed = AtomicBool::new(false);
        {
            let future = async {
                let mut window = Window::new(&observed, true);
                std::future::pending::<()>().await;
                window.complete();
            };
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(1), future)
                    .await
                    .is_err()
            );
        }
        assert!(observed.load(Ordering::Relaxed));
        observed.store(false, Ordering::Relaxed);
        {
            let mut window = Window::new(&observed, true);
            window.complete();
        }
        assert!(!observed.load(Ordering::Relaxed));
    }
}
