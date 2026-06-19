//! Single source of truth for the Cairo-drawn dashboard colours (gauges, meters,
//! charts, sparklines). Every drawn widget reads its threshold colours from here,
//! so a metric can never show two different greens. The CSS chrome (panel
//! borders, headers) keeps its own accent in `ui/style.css`; keep the two in the
//! same colour family when retuning either.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Rgb {
    pub const fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b }
    }

    pub fn lerp(self, other: Self, k: f64) -> Self {
        let k = k.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * k,
            g: self.g + (other.g - self.g) * k,
            b: self.b + (other.b - self.b) * k,
        }
    }
}

/// Semantic threshold colours: green good, amber warning, red critical.
pub const GOOD: Rgb = Rgb::new(0.27, 0.84, 0.17);
pub const WARN: Rgb = Rgb::new(1.0, 0.72, 0.0);
pub const CRIT: Rgb = Rgb::new(1.0, 0.25, 0.25);

/// Fraction at which a "higher is worse" metric turns from good to warning,
/// and from warning to critical. Shared by every ramp so the bands line up.
pub const THRESH_WARN: f64 = 0.6;
pub const THRESH_CRIT: f64 = 0.85;

/// Continuous good->warn->crit ramp for "higher is worse" metrics (gauges,
/// charts). Knots sit on the shared thresholds so it agrees with [`band`].
pub fn ramp(frac: f64) -> Rgb {
    let f = frac.clamp(0.0, 1.0);
    if f <= THRESH_WARN {
        GOOD.lerp(WARN, f / THRESH_WARN)
    } else if f <= THRESH_CRIT {
        WARN.lerp(CRIT, (f - THRESH_WARN) / (THRESH_CRIT - THRESH_WARN))
    } else {
        CRIT
    }
}

/// Discrete good/warn/crit bands for segmented readouts (bars, LED meters).
pub fn band(frac: f64) -> Rgb {
    let f = frac.clamp(0.0, 1.0);
    if f < THRESH_WARN {
        GOOD
    } else if f < THRESH_CRIT {
        WARN
    } else {
        CRIT
    }
}

/// Battery charge colour: here a *low* value is the bad one, so the bands are
/// inverted relative to load/temperature.
pub fn charge(frac: f64) -> Rgb {
    let f = frac.clamp(0.0, 1.0);
    if f < 0.2 {
        CRIT
    } else if f < 0.4 {
        WARN
    } else {
        GOOD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_endpoints_and_midpoint() {
        let a = Rgb::new(0.0, 0.0, 0.0);
        let b = Rgb::new(1.0, 1.0, 1.0);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(a.lerp(b, 0.5), Rgb::new(0.5, 0.5, 0.5));
    }

    #[test]
    fn lerp_clamps_out_of_range() {
        let a = Rgb::new(0.0, 0.0, 0.0);
        let b = Rgb::new(1.0, 1.0, 1.0);
        assert_eq!(a.lerp(b, -1.0), a);
        assert_eq!(a.lerp(b, 2.0), b);
    }

    #[test]
    fn ramp_agrees_with_band_at_knots() {
        assert_eq!(ramp(0.0), GOOD);
        assert_eq!(ramp(THRESH_WARN), WARN);
        assert_eq!(ramp(THRESH_CRIT), CRIT);
        assert_eq!(ramp(1.0), CRIT);
    }

    #[test]
    fn ramp_channels_stay_in_unit_range() {
        for i in 0..=100 {
            let c = ramp(f64::from(i) / 100.0);
            for ch in [c.r, c.g, c.b] {
                assert!((0.0..=1.0).contains(&ch), "channel {ch} out of range");
            }
        }
    }

    #[test]
    fn band_steps_on_thresholds() {
        assert_eq!(band(0.0), GOOD);
        assert_eq!(band(0.59), GOOD);
        assert_eq!(band(0.6), WARN);
        assert_eq!(band(0.84), WARN);
        assert_eq!(band(0.85), CRIT);
        assert_eq!(band(1.0), CRIT);
    }

    #[test]
    fn charge_is_inverted() {
        assert_eq!(charge(0.1), CRIT);
        assert_eq!(charge(0.3), WARN);
        assert_eq!(charge(0.9), GOOD);
    }
}
