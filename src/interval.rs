use futures::{Future, Stream, Async, Poll};

use {Sleep, TimerError};

use std::time::Duration;

use rand::{thread_rng, Rng};

/// A stream representing notifications at given interval
///
/// Intervals are created through `Timer::interval`.
#[derive(Debug)]
pub struct Interval {
    sleep: Sleep,
    min_duration: Duration,
    max_duration: Duration,
}

/// Create a new interval
pub fn new(sleep: Sleep, min_dur: Duration, max_dur: Duration) -> Interval {
    Interval {
        sleep: sleep,
        min_duration: min_dur,
        max_duration: max_dur,
    }
}

const NANOS_PER_SEC: u32 = 1_000_000_000;

/// Returns the next duration for an interval
/// If `min` and `max` are equal, the duration is fixed.
/// If `min` and `max` are not equal, a duration in the range [`min`, `max`] is returned.
///
/// # Panics
///
/// Panics if `max < min`.
pub(crate) fn next_duration(min: Duration, max: Duration) -> Duration {
    let mut rng = thread_rng();

    let secs = if min.as_secs() == max.as_secs() {
        min.as_secs()
    } else {
        rng.gen_range(min.as_secs(), max.as_secs() + 1)
    };

    let nsecs = if min.subsec_nanos() == max.subsec_nanos() {
        min.subsec_nanos()
    } else if secs == min.as_secs() {
        rng.gen_range(min.subsec_nanos(), NANOS_PER_SEC)
    } else if secs == max.as_secs() {
        rng.gen_range(0, max.subsec_nanos() + 1)
    } else {
        rng.gen_range(0, NANOS_PER_SEC)
    };

    Duration::new(secs, nsecs)
}

impl Stream for Interval {
    type Item = ();
    type Error = TimerError;

    fn poll(&mut self) -> Poll<Option<()>, TimerError> {
        let _ = try_ready!(self.sleep.poll());

        // Reset the timeout
        self.sleep = self.sleep.timer().sleep(next_duration(self.min_duration, self.max_duration));

        Ok(Async::Ready(Some(())))
    }
}
