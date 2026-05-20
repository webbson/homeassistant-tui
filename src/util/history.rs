use std::collections::VecDeque;
use std::time::Instant;

/// Bounded ring buffer of (time, value) samples.
#[derive(Debug, Clone)]
pub struct RingBuf {
    buf: VecDeque<(Instant, f64)>,
    cap: usize,
}

impl RingBuf {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(cap),
            cap,
        }
    }

    pub fn push(&mut self, t: Instant, v: f64) {
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back((t, v));
    }

    /// Replace contents with a backfilled, time-ordered series.
    pub fn fill_from(&mut self, values: impl IntoIterator<Item = f64>) {
        self.buf.clear();
        let now = Instant::now();
        for v in values {
            if self.buf.len() == self.cap {
                self.buf.pop_front();
            }
            self.buf.push_back((now, v));
        }
    }

    pub fn values(&self) -> impl Iterator<Item = f64> + '_ {
        self.buf.iter().map(|(_, v)| *v)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn compute(&self, metric: crate::dashboard::StatsMetric) -> Option<f64> {
        let vals: Vec<f64> = self.values().collect();
        if vals.is_empty() {
            return None;
        }
        use crate::dashboard::StatsMetric::*;
        Some(match metric {
            Min => vals.iter().cloned().fold(f64::INFINITY, f64::min),
            Max => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            Sum => vals.iter().sum(),
            Avg => vals.iter().sum::<f64>() / vals.len() as f64,
            Count => vals.len() as f64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn evicts_oldest() {
        let mut r = RingBuf::new(3);
        let t = Instant::now();
        r.push(t, 1.0);
        r.push(t + Duration::from_secs(1), 2.0);
        r.push(t + Duration::from_secs(2), 3.0);
        r.push(t + Duration::from_secs(3), 4.0);
        let vs: Vec<f64> = r.values().collect();
        assert_eq!(vs, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn metric_min_max_avg_sum_count() {
        use crate::dashboard::StatsMetric;
        let mut b = RingBuf::new(64);
        let t = Instant::now();
        for (i, v) in [1.0, 2.0, 3.0, 4.0].iter().enumerate() {
            b.push(t + std::time::Duration::from_secs(i as u64), *v);
        }
        assert_eq!(b.compute(StatsMetric::Min), Some(1.0));
        assert_eq!(b.compute(StatsMetric::Max), Some(4.0));
        assert_eq!(b.compute(StatsMetric::Avg), Some(2.5));
        assert_eq!(b.compute(StatsMetric::Sum), Some(10.0));
        assert_eq!(b.compute(StatsMetric::Count), Some(4.0));
    }

    #[test]
    fn metric_empty_buffer_returns_none() {
        use crate::dashboard::StatsMetric;
        let b = RingBuf::new(64);
        assert_eq!(b.compute(StatsMetric::Avg), None);
    }
}
