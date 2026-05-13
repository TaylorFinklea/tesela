//! Hybrid logical clock.
//!
//! Thin wrapper around [`uhlc::HLC`] for monotonicity, clock-skew detection,
//! and serde-friendly timestamps. The wrapper is intentional so a future
//! replacement (in-house implementation, alternate library) is mechanical.

mod timestamp;

pub use timestamp::HlcTimestamp;

use crate::device::DeviceId;
use crate::error::{SyncError, SyncResult};
use std::sync::Arc;
use std::time::Duration;

/// Default max clock drift accepted from remote timestamps: 5 seconds.
///
/// Larger than uhlc's default of 100ms because macOS-to-iPhone first-boot
/// skew can be noticeable. Tunable per `Hlc::with_max_drift`.
pub const DEFAULT_MAX_DRIFT_MILLIS: i64 = 5_000;

/// Hybrid logical clock with monotonicity guard and drift bound.
#[derive(Clone)]
pub struct Hlc {
    inner: Arc<uhlc::HLC>,
    max_drift_millis: i64,
    device: DeviceId,
}

impl Hlc {
    /// New HLC bound to a device id, using the default max drift.
    pub fn new(device: DeviceId) -> Self {
        Self::with_max_drift(device, DEFAULT_MAX_DRIFT_MILLIS)
    }

    /// New HLC bound to a device id, with a custom max drift in millis.
    pub fn with_max_drift(device: DeviceId, max_drift_millis: i64) -> Self {
        let id = uhlc::ID::try_from(device.as_bytes().as_slice())
            .expect("DeviceId is 16 bytes, fits in uhlc::ID");
        let inner = uhlc::HLCBuilder::new()
            .with_id(id)
            .with_max_delta(Duration::from_millis(max_drift_millis as u64))
            .build();
        Self {
            inner: Arc::new(inner),
            max_drift_millis,
            device,
        }
    }

    /// Tick the clock and return the new timestamp. Strictly monotonic
    /// even if the system wall clock goes backward.
    pub fn now(&self) -> HlcTimestamp {
        let ts = self.inner.new_timestamp();
        HlcTimestamp::from_uhlc(ts, self.device)
    }

    /// Observe a remote timestamp and advance our clock to be strictly
    /// greater. Rejects timestamps too far in the future.
    pub fn observe(&self, remote: HlcTimestamp) -> SyncResult<HlcTimestamp> {
        let remote_uhlc = remote.to_uhlc();
        match self.inner.update_with_timestamp(&remote_uhlc) {
            Ok(()) => Ok(self.now()),
            Err(err) => {
                let msg = err.to_string();
                // uhlc's error doesn't expose drift directly. Compute it
                // ourselves so the error variant carries useful numbers.
                let now_ms = chrono::Utc::now().timestamp_millis();
                let drift = remote.physical_millis() - now_ms;
                if drift > self.max_drift_millis {
                    Err(SyncError::ClockSkew {
                        drift_millis: drift,
                        max_drift_millis: self.max_drift_millis,
                    })
                } else {
                    Err(SyncError::Protocol(msg))
                }
            }
        }
    }

    /// The device id this clock is bound to.
    pub fn device(&self) -> DeviceId {
        self.device
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceId;

    #[test]
    fn monotonic_across_calls() {
        let hlc = Hlc::new(DeviceId::new_random());
        let mut prev = hlc.now();
        for _ in 0..1000 {
            let next = hlc.now();
            assert!(next > prev, "HLC went backward: {prev:?} then {next:?}");
            prev = next;
        }
    }

    fn build_remote_ts(offset_millis: i64, device: DeviceId) -> HlcTimestamp {
        // Build an NTP64 word at `chrono::Utc::now() + offset_millis`.
        let target_millis = chrono::Utc::now().timestamp_millis() + offset_millis;
        let secs = (target_millis / 1000) as u64;
        let rem_ms = (target_millis % 1000).max(0) as u64;
        let frac = (rem_ms << 32) / 1000;
        let ntp64 = (secs << 32) | (frac & 0xFFFF_FFFF);
        HlcTimestamp { ntp64, device }
    }

    #[test]
    fn observe_advances_physical_to_remote() {
        let local = Hlc::new(DeviceId::new_random());
        let remote_device = DeviceId::new_random();
        let now_ms = chrono::Utc::now().timestamp_millis();
        // Build a remote ts ~500ms in the future. uhlc max_delta=5s allows it.
        let future = build_remote_ts(500, remote_device);
        let merged = local.observe(future).expect("within drift bound");
        assert!(merged.physical_millis() >= now_ms + 400);
    }

    #[test]
    fn observe_rejects_skew_over_max_drift() {
        let local = Hlc::with_max_drift(DeviceId::new_random(), 100);
        let remote_device = DeviceId::new_random();
        let too_far = build_remote_ts(10_000, remote_device);
        let err = local.observe(too_far).expect_err("should reject");
        match err {
            SyncError::ClockSkew { drift_millis, .. } => {
                assert!(drift_millis >= 9_000, "drift_millis={drift_millis}");
            }
            other => panic!("expected ClockSkew, got {other:?}"),
        }
    }
}
