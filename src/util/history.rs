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
}
