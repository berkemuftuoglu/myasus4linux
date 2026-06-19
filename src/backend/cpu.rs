//! Per-thread CPU frequency and load, for the live cooling dashboard.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreStat {
    pub id: usize,
    pub mhz: u32,
    /// Utilisation since the previous sample, 0.0 to 100.0.
    pub load: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

/// Holds the previous `/proc/stat` snapshot so load can be computed as a delta
/// between two `sample()` calls.
#[derive(Debug, Default)]
pub struct CpuMonitor {
    prev: Vec<CpuTimes>,
}

impl CpuMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sample(&mut self) -> Vec<CoreStat> {
        let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
        let times = parse_cpu_times(&stat);

        let stats = times
            .iter()
            .enumerate()
            .map(|(i, &cur)| CoreStat {
                id: i,
                mhz: read_freq_mhz(i).unwrap_or(0),
                load: self.prev.get(i).map_or(0.0, |&p| load_between(p, cur)),
            })
            .collect();

        self.prev = times;
        stats
    }
}

fn parse_cpu_times(stat: &str) -> Vec<CpuTimes> {
    stat.lines()
        .filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(u8::is_ascii_digit))
        .map(|line| {
            let fields: Vec<u64> = line
                .split_whitespace()
                .skip(1)
                .filter_map(|n| n.parse().ok())
                .collect();
            // user nice system idle iowait irq softirq steal guest guest_nice
            let idle = fields.get(3).copied().unwrap_or(0) + fields.get(4).copied().unwrap_or(0);
            CpuTimes {
                idle,
                total: fields.iter().sum(),
            }
        })
        .collect()
}

fn load_between(prev: CpuTimes, cur: CpuTimes) -> f64 {
    let d_total = cur.total.saturating_sub(prev.total);
    if d_total == 0 {
        return 0.0;
    }
    let d_idle = cur.idle.saturating_sub(prev.idle);
    let busy = d_total.saturating_sub(d_idle) as f64 / d_total as f64;
    (busy * 100.0).clamp(0.0, 100.0)
}

fn read_freq_mhz(cpu: usize) -> Option<u32> {
    let path = format!("/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_cur_freq");
    let khz: u32 = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
    Some(khz / 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_per_cpu_lines_only() {
        let stat = "cpu  100 0 100 800 0 0 0 0 0 0\n\
                    cpu0 10 0 10 80 0 0 0 0 0 0\n\
                    cpu1 20 0 20 60 0 0 0 0 0 0\n\
                    intr 12345\n";
        let times = parse_cpu_times(stat);
        assert_eq!(times.len(), 2); // aggregate "cpu " excluded
        assert_eq!(
            times[0],
            CpuTimes {
                idle: 80,
                total: 100
            }
        );
    }

    #[test]
    fn load_is_busy_fraction_between_samples() {
        // 100 total jiffies elapsed, 20 of them idle -> 80% busy
        let prev = CpuTimes { idle: 0, total: 0 };
        let cur = CpuTimes {
            idle: 20,
            total: 100,
        };
        assert!((load_between(prev, cur) - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn load_zero_when_no_time_elapsed() {
        let same = CpuTimes {
            idle: 50,
            total: 200,
        };
        assert!(load_between(same, same).abs() < f64::EPSILON);
    }
}
