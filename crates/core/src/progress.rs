//! A copy's progress: files and bytes done, out of the files to copy.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::Serialize;

/// Where a copy stands. Bytes count the sizes of the files to copy; a file that fails or
/// isn't tried counts as done, so the bar always reaches its end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub files_done: usize,
    pub files_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

/// What a front end shows: the progress and, once known, the time left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Report {
    pub progress: Progress,
    /// Seconds left at the recent speed; `None` for the first `WARM_UP`.
    pub secs_left: Option<u64>,
}

/// Turns the copy's many progress calls (one per megabyte) into reports for a screen: at
/// most one every `TICK`, and always the last one, with the time left at the speed of the
/// last `WINDOW`.
#[derive(Debug)]
pub struct Meter {
    start: Instant,
    last_sent: Option<Instant>,
    /// (time, bytes done) of the reports sent within `WINDOW`, oldest first; the start
    /// until then.
    samples: VecDeque<(Instant, u64)>,
}

/// The shortest time between two reports.
pub const TICK: Duration = Duration::from_millis(100);
/// No time left before this: the first seconds' speed says little.
pub const WARM_UP: Duration = Duration::from_secs(3);
/// The speed is measured over this last stretch, so it follows a slower card or disk.
pub const WINDOW: Duration = Duration::from_secs(5);

impl Meter {
    /// A meter for a copy started at `start`.
    pub fn new(start: Instant) -> Self {
        Meter {
            start,
            last_sent: None,
            samples: VecDeque::from([(start, 0)]),
        }
    }

    /// The report to show for `progress` at `now`, if one is due.
    pub fn update(&mut self, now: Instant, progress: Progress) -> Option<Report> {
        let finished = progress.files_done == progress.files_total;
        let due = self
            .last_sent
            .is_none_or(|last| now.duration_since(last) >= TICK);
        if !(due || finished) {
            return None;
        }
        self.last_sent = Some(now);
        while self.samples.len() > 1
            && self
                .samples
                .front()
                .is_some_and(|(t, _)| now.duration_since(*t) > WINDOW)
        {
            self.samples.pop_front();
        }
        let secs_left = self.secs_left(now, progress);
        self.samples.push_back((now, progress.bytes_done));
        Some(Report {
            progress,
            secs_left,
        })
    }

    fn secs_left(&self, now: Instant, progress: Progress) -> Option<u64> {
        let finished = progress.bytes_done >= progress.bytes_total;
        if finished || now.duration_since(self.start) < WARM_UP {
            return None;
        }
        let (since, bytes_then) = *self.samples.front()?;
        let bytes = u128::from(progress.bytes_done.checked_sub(bytes_then)?);
        let nanos = now.duration_since(since).as_nanos();
        if bytes == 0 || nanos == 0 {
            return None;
        }
        let left = u128::from(progress.bytes_total.saturating_sub(progress.bytes_done));
        u64::try_from(left * nanos / bytes / 1_000_000_000).ok()
    }
}
