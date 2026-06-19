//! Cancellation + progress primitives shared by long-running scans/cleans.
//!
//! These are intentionally toolkit-agnostic: a frontend passes a [`Progress`]
//! callback and holds a [`CancelToken`] it can flip when the user hits Cancel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cheap, cloneable cancellation flag. Clone it into a worker thread; flip it
/// from the UI thread with [`CancelToken::cancel`].
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation. Idempotent.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A progress update emitted during a scan or clean.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// 0.0..=1.0 when known, or `None` for indeterminate work.
    pub fraction: Option<f64>,
    /// Short status line, e.g. "Scanning ~/.cache…".
    pub message: String,
}

/// A boxed progress callback. Frontends marshal these onto their UI thread.
pub type Progress<'a> = dyn FnMut(ProgressUpdate) + Send + 'a;

/// Convenience to emit a progress update through an optional callback.
pub fn report(cb: &mut Option<&mut Progress<'_>>, fraction: Option<f64>, message: impl Into<String>) {
    if let Some(cb) = cb.as_mut() {
        cb(ProgressUpdate {
            fraction,
            message: message.into(),
        });
    }
}
