use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Latest microphone RMS level, shared with the UI without locking the audio callback.
#[derive(Clone, Default)]
pub struct InputLevel {
    bits: Arc<AtomicU32>,
}

impl InputLevel {
    pub fn rms(&self) -> f32 {
        f32::from_bits(self.bits.load(Ordering::Relaxed))
    }

    pub(super) fn publish_rms(&self, rms: f32) {
        self.bits.store(rms.to_bits(), Ordering::Relaxed);
    }
}
