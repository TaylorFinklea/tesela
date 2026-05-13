//! Serde-friendly HLC timestamp type.

use crate::device::DeviceId;
use serde::{Deserialize, Serialize};

/// HLC timestamp.
///
/// Stores the full `uhlc::NTP64` value as a single u64 (rather than
/// splitting into physical and logical halves) because the logical counter
/// is encoded into the bottom of the NTP64 fraction by uhlc, not into a
/// separate field. Splitting it loses the meaning. Treating the NTP64 as
/// the single ordering key matches what uhlc itself does internally.
///
/// Ordering is lexicographic over `(ntp64, device)`. The device tie-break
/// gives a total order across devices that share an NTP64 word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HlcTimestamp {
    /// Full NTP64 word as produced by `uhlc::HLC::new_timestamp`. Top 32
    /// bits are NTP seconds since 1970; bottom 32 bits are a fraction of
    /// a second plus uhlc's logical counter bump on ties.
    pub ntp64: u64,
    /// Device that authored the timestamp.
    pub device: DeviceId,
}

impl HlcTimestamp {
    /// Build from a `uhlc::Timestamp` plus the device id. uhlc's `ID`
    /// type does not round-trip cleanly to a fixed-size array on all
    /// versions, so we pass the device id explicitly.
    pub fn from_uhlc(ts: uhlc::Timestamp, device: DeviceId) -> Self {
        let ntp64 = ts.get_time().as_u64();
        Self { ntp64, device }
    }

    /// Convert back to a `uhlc::Timestamp`.
    pub fn to_uhlc(&self) -> uhlc::Timestamp {
        let id = uhlc::ID::try_from(self.device.as_bytes().as_slice())
            .expect("DeviceId is 16 bytes");
        uhlc::Timestamp::new(uhlc::NTP64(self.ntp64), id)
    }

    /// Approximate physical time as milliseconds since the Unix epoch.
    /// Used for clock-skew bounds and human-readable timestamps. NOT used
    /// for ordering (ordering uses [`Self::ntp64`] directly).
    pub fn physical_millis(&self) -> i64 {
        let secs = (self.ntp64 >> 32) as i64;
        let frac = (self.ntp64 & 0xFFFF_FFFF) as u64;
        let millis_in_frac = ((frac * 1000) >> 32) as i64;
        secs * 1000 + millis_in_frac
    }

    /// NTP64 reinterpreted as i64 for SQLite storage. All reasonable
    /// NTP64 values have the top bit clear (seconds < 2^31), so signed
    /// vs unsigned comparison agrees.
    pub fn ntp64_as_i64(&self) -> i64 {
        self.ntp64 as i64
    }

    /// Inverse of [`Self::ntp64_as_i64`].
    pub fn from_ntp64_i64(ntp: i64, device: DeviceId) -> Self {
        Self {
            ntp64: ntp as u64,
            device,
        }
    }
}

impl Ord for HlcTimestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ntp64
            .cmp(&other.ntp64)
            .then(self.device.0.cmp(&other.device.0))
    }
}

impl PartialOrd for HlcTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ord_lexicographic() {
        let dev = DeviceId::new_random();
        let a = HlcTimestamp { ntp64: 100, device: dev };
        let b = HlcTimestamp { ntp64: 101, device: dev };
        let c = HlcTimestamp { ntp64: 200, device: dev };
        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    #[test]
    fn device_tiebreak_provides_total_order() {
        let dev_a = DeviceId::from_bytes([0x00; 16]);
        let dev_b = DeviceId::from_bytes([0xff; 16]);
        let ta = HlcTimestamp { ntp64: 1000, device: dev_a };
        let tb = HlcTimestamp { ntp64: 1000, device: dev_b };
        assert!(ta < tb);
    }

    #[test]
    fn physical_millis_in_reasonable_range() {
        let now_uhlc = uhlc::HLC::default().new_timestamp();
        let ts = HlcTimestamp::from_uhlc(now_uhlc, DeviceId::new_random());
        let now_chrono = chrono::Utc::now().timestamp_millis();
        // Within 10 seconds of system clock.
        let diff = (ts.physical_millis() - now_chrono).abs();
        assert!(diff < 10_000, "physical_millis off by {diff}");
    }
}
