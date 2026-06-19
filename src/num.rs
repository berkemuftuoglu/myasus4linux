//! The few f64 -> integer rounds the UI needs (durations, slider values),
//! clamped into range so they can't truncate or lose a sign. Centralising them
//! here is what lets the cast lints stay denied everywhere else instead of a
//! blanket repo-wide allow.

/// Round and clamp an f64 into a `u32`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "value is clamped into [0, u32::MAX] immediately before the cast"
)]
pub fn round_u32(x: f64) -> u32 {
    x.round().clamp(0.0, f64::from(u32::MAX)) as u32
}

/// Round and clamp an f64 into a `u8`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "value is clamped into [0, u8::MAX] immediately before the cast"
)]
pub fn round_u8(x: f64) -> u8 {
    x.round().clamp(0.0, f64::from(u8::MAX)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_u32_rounds_and_clamps() {
        assert_eq!(round_u32(3.4), 3);
        assert_eq!(round_u32(3.6), 4);
        assert_eq!(round_u32(-5.0), 0);
        assert_eq!(round_u32(1e30), u32::MAX);
    }

    #[test]
    fn round_u8_rounds_and_clamps() {
        assert_eq!(round_u8(79.5), 80);
        assert_eq!(round_u8(-1.0), 0);
        assert_eq!(round_u8(999.0), 255);
    }
}
