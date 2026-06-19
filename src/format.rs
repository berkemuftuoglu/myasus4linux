//! Small pure formatting/parsing helpers shared across pages. Kept free of GTK
//! and of I/O so they can be unit-tested directly.

/// Render an optional duration in hours as `Hh MMm`, or an em dash when
/// unknown or non-finite. Shared by the battery and overview pages.
pub fn duration_hm(hours: Option<f64>) -> String {
    match hours {
        Some(h) if h.is_finite() && h > 0.0 => {
            let mins = crate::num::round_u32(h * 60.0);
            format!("{}h {:02}m", mins / 60, mins % 60)
        }
        _ => "—".to_owned(),
    }
}

/// Pull a `/proc/meminfo`-style value (in kB) for the given key prefix, e.g.
/// `meminfo_kb(contents, "MemTotal:")`.
pub fn meminfo_kb(contents: &str, key: &str) -> Option<u64> {
    contents
        .lines()
        .find(|l| l.starts_with(key))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_formats_hours_and_minutes() {
        assert_eq!(duration_hm(Some(1.5)), "1h 30m");
        assert_eq!(duration_hm(Some(0.25)), "0h 15m");
        assert_eq!(duration_hm(Some(10.0)), "10h 00m");
    }

    #[test]
    fn duration_handles_unknown_and_garbage() {
        assert_eq!(duration_hm(None), "—");
        assert_eq!(duration_hm(Some(0.0)), "—");
        assert_eq!(duration_hm(Some(-3.0)), "—");
        assert_eq!(duration_hm(Some(f64::INFINITY)), "—");
        assert_eq!(duration_hm(Some(f64::NAN)), "—");
    }

    #[test]
    fn meminfo_reads_named_key() {
        let sample = "MemTotal:       16077216 kB\nMemAvailable:    9123456 kB\n";
        assert_eq!(meminfo_kb(sample, "MemTotal:"), Some(16_077_216));
        assert_eq!(meminfo_kb(sample, "MemAvailable:"), Some(9_123_456));
        assert_eq!(meminfo_kb(sample, "Nope:"), None);
    }
}
