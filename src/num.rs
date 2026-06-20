//! The few f64 -> integer rounds the UI needs (durations, slider values),
//! clamped into range so they can't truncate or lose a sign. Centralising the
//! truncating and sign-changing casts here keeps `cast_possible_truncation` and
//! `cast_sign_loss` denied everywhere else; only `cast_precision_loss` (a small
//! int widening to f64) stays allowed repo-wide, since it's always exact here.

/// Round and clamp an f64 into a `u32`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "value is clamped into [0, u32::MAX] immediately before the cast"
)]
pub fn round_u32(x: f64) -> u32 {
    x.round().clamp(0.0, f64::from(u32::MAX)) as u32
}

/// Round and clamp an f64 into a `u8` within `[lo, hi]`. Callers pass the
/// control's real range, so an out-of-range slider value clamps to the control's
/// max rather than silently to 255.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "value is clamped into [lo, hi] <= u8::MAX immediately before the cast"
)]
pub fn round_u8_in(x: f64, lo: u8, hi: u8) -> u8 {
    x.round().clamp(f64::from(lo), f64::from(hi)) as u8
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
    fn round_u8_in_rounds_and_clamps_to_range() {
        assert_eq!(round_u8_in(79.5, 40, 100), 80);
        assert_eq!(round_u8_in(10.0, 40, 100), 40); // below the control's min
        assert_eq!(round_u8_in(999.0, 0, 3), 3); // clamped to the real max, not 255
    }
}
