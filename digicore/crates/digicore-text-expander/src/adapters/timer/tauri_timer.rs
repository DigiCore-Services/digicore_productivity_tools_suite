//! TauriTimerAdapter - TimerPort for Tauri GUI.
//!
//! Uses a channel to signal repaint requests. The Tauri app polls the channel
//! and emits events to the frontend when a repaint is requested.
//! Framework-agnostic: works without the Tauri crate.
//!
//! Only compiled when feature `gui-tauri` is enabled.

use crate::ports::TimerPort;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

/// Timer adapter that sends repaint requests on a channel after the given duration.
pub struct TauriTimerAdapter {
    tx: mpsc::Sender<()>,
}

impl TauriTimerAdapter {
    /// Create adapter that sends to the given channel when repaint is requested.
    pub fn new(tx: mpsc::Sender<()>) -> Self {
        Self { tx }
    }

    /// Create adapter and return the receiver for the app to poll.
    pub fn with_channel() -> (Self, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel();
        (Self::new(tx), rx)
    }
}

impl TimerPort for TauriTimerAdapter {
    fn schedule_repaint_after(&self, duration: Duration) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            thread::sleep(duration);
            let _ = tx.send(());
        });
    }
}

/// Shared repaint request sender - allows creating TimerPort from app context.
#[derive(Clone)]
pub struct TauriTimerContext {
    tx: Arc<Mutex<mpsc::Sender<()>>>,
}

impl TauriTimerContext {
    pub fn new(tx: mpsc::Sender<()>) -> Self {
        Self {
            tx: Arc::new(Mutex::new(tx)),
        }
    }

    pub fn create_timer(&self) -> TauriTimerAdapter {
        TauriTimerAdapter::new(self.tx.lock().unwrap().clone())
    }
}
