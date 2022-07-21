use std::fmt;

use serde_derive::{Deserialize, Serialize};
use time::Time;

time::serde::format_description!(hm_time, Time, "[hour]:[minute]");

/// Used for limiting the running time.
///
/// Note: Limiting the time only works for scheduled tasks!
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub(crate) struct TimeLimits {
    #[serde(with = "hm_time")]
    start_time: Time,
    #[serde(with = "hm_time")]
    end_time: Time,
}

impl TimeLimits {
    pub(crate) fn is_within_limits(&self, time: &Time) -> bool {
        if self.start_time < self.end_time {
            time >= &self.start_time && time <= &self.end_time
        } else {
            time >= &self.start_time || time <= &self.end_time
        }
    }
}

impl fmt::Display for TimeLimits {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:02}:{:02}–{:02}:{:02}",
            self.start_time.hour(),
            self.start_time.minute(),
            self.end_time.hour(),
            self.end_time.minute()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::time;
    #[test]
    fn time_limits_simple() {
        let limits = TimeLimits {
            start_time: time!(8:00),
            end_time: time!(12:30),
        };

        let tm_before = time!(7:59);
        let tm_on = time!(8:00);
        let tm_within1 = time!(9:30);
        let tm_within2 = time!(12:28);
        let tm_after = time!(12:32);

        assert_eq!(limits.is_within_limits(&tm_before), false);
        assert_eq!(limits.is_within_limits(&tm_on), true);
        assert_eq!(limits.is_within_limits(&tm_within1), true);
        assert_eq!(limits.is_within_limits(&tm_within2), true);
        assert_eq!(limits.is_within_limits(&tm_after), false);
    }

    #[test]
    fn time_limits_complex() {
        let limits = TimeLimits {
            start_time: time!(22:00),
            end_time: time!(2:30),
        };

        let tm_before = time!(21:00);
        let tm_on1 = time!(22:00);
        let tm_on2 = time!(2:30);
        let tm_within1 = time!(23:30);
        let tm_within2 = time!(0:00);
        let tm_within3 = time!(1:59);
        let tm_after = time!(3:00);

        assert_eq!(limits.is_within_limits(&tm_before), false);
        assert_eq!(limits.is_within_limits(&tm_on1), true);
        assert_eq!(limits.is_within_limits(&tm_on2), true);
        assert_eq!(limits.is_within_limits(&tm_within1), true);
        assert_eq!(limits.is_within_limits(&tm_within2), true);
        assert_eq!(limits.is_within_limits(&tm_within3), true);
        assert_eq!(limits.is_within_limits(&tm_after), false);
    }
}
