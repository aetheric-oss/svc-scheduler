//! Provides calendar/scheduling utilities
//! Parses and serializes string RRULEs with duration and provides api to query if time slot is available.

use iso8601_duration::Duration as Iso8601Duration;
use lib_common::time::{chrono_tz, DateTime, Duration, NaiveDateTime, TimeZone, Utc};
pub use rrule::{RRuleSet, Tz as RRuleTz};
use std::cmp::{max, min};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::ops::Sub;
use std::str::FromStr;

/// The maximum number of events to return from an RRULE
const MAX_EVENT_COUNT: u16 = 100;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Timeslot {
    pub time_start: DateTime<Utc>,
    pub time_end: DateTime<Utc>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TimeslotError {
    NoOverlap,
}

impl Timeslot {
    pub fn duration(&self) -> Duration {
        self.time_end - self.time_start
    }

    pub fn split(&self, min_duration: &Duration, max_duration: &Duration) -> Vec<Timeslot> {
        let mut slots = vec![];
        let mut current_time = self.time_start;

        while current_time < self.time_end {
            let next_time = min(current_time + *max_duration, self.time_end);
            let timeslot = Timeslot {
                time_start: current_time,
                time_end: next_time,
            };

            if timeslot.duration() >= *min_duration {
                slots.push(timeslot);
            }

            current_time = next_time;
        }

        slots
    }

    pub fn overlap(&self, other: &Self) -> Result<Self, TimeslotError> {
        //
        //               |      self           |
        //         |        other         |
        //               |   overlap      |
        let slot = Self {
            time_start: max(self.time_start, other.time_start),
            time_end: min(self.time_end, other.time_end),
        };

        if slot.time_start >= slot.time_end {
            return Err(TimeslotError::NoOverlap);
        }

        Ok(slot)
    }
}

impl Sub for Timeslot {
    type Output = Vec<Timeslot>;

    fn sub(self, other: Self) -> Self::Output {
        // Occupied slot ends before available slot starts
        //  or occupied slot starts after available slot ends
        if self.time_end <= other.time_start || self.time_start >= other.time_end {
            return vec![self];
        }

        // Occupied slot starts before and ends after available slot
        // |           OCCUPIED           |
        //                +
        //     | Available | Available |
        //                =
        //       (no available slots)
        if other.time_start <= self.time_start && other.time_end >= self.time_end {
            // other timeslot obliterates this available timeslot
            return vec![];
        }

        // Occupied slot right in the middle of the available slot, so we need to split the available slot
        //       | Occupied |
        //            +
        // |     Available          |
        //            =
        // | Av. |           | Av.  |
        if self.time_start < other.time_start && self.time_end > other.time_end {
            return vec![
                Timeslot {
                    time_start: self.time_start,
                    time_end: other.time_start,
                },
                Timeslot {
                    time_start: other.time_end,
                    time_end: self.time_end,
                },
            ];
        }

        //        | Occupied |
        //       +
        // | Available |
        //       =
        //  | Av. |
        if self.time_start < other.time_start && self.time_end <= other.time_end {
            return vec![Timeslot {
                time_start: self.time_start,
                time_end: other.time_start,
            }];
        }

        //
        // |     Occupied     |
        //            +
        //      |     Available      |
        //            =
        //                     | Av. |
        if self.time_start >= other.time_start && self.time_end > other.time_end {
            return vec![Timeslot {
                time_start: other.time_end,
                time_end: self.time_end,
            }];
        }

        router_warn!("Unhandled case: {:?} {:?}", self, other);

        vec![]
    }
}

// /// formats DateTime to string in format: `YYYYMMDDThhmmssZ`, e.g. 20221026T133000Z
fn datetime_to_ical_format(dt: &DateTime<RRuleTz>) -> String {
    router_debug!("{:?}", dt);
    let mut tz_prefix = String::new();
    let mut tz_postfix = String::new();
    router_debug!("tz: {:?}", dt.timezone());
    let tz = dt.timezone();
    match tz {
        RRuleTz::Local(_) => {}
        RRuleTz::Tz(tz) => match tz {
            chrono_tz::Tz::UTC => {
                tz_postfix = "Z".to_string();
            }
            tz => {
                tz_prefix = format!(";TZID={}:", tz.name());
            }
        },
    }

    let dt = dt.format("%Y%m%dT%H%M%S");
    router_debug!("dt: {:?}", dt);
    format!("{}{}{}", tz_prefix, dt, tz_postfix)
}

/// Wraps rruleset and their duration
#[derive(Debug, Clone)]
pub struct RecurrentEvent {
    /// The rruleset with recurrence rules
    pub rrule_set: RRuleSet,
    /// The duration of the event
    pub duration: Duration,
}
///Calendar implementation for recurring events using the rrule crate and duration iso8601_duration crate
#[derive(Debug, Clone)]
pub struct Calendar {
    ///Vec of rrulesets and their duration
    pub events: Vec<RecurrentEvent>,
}

#[derive(Debug, Copy, Clone)]
pub enum CalendarError {
    Rrule,
    RruleSet,
    HeaderPartsLength,
    Duration,
    Internal,
}

impl Display for CalendarError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            CalendarError::Rrule => write!(f, "Invalid rrule"),
            CalendarError::RruleSet => write!(f, "Invalid rrule set"),
            CalendarError::HeaderPartsLength => write!(f, "Invalid header parts length"),
            CalendarError::Duration => write!(f, "Invalid duration"),
            CalendarError::Internal => write!(f, "Internal error"),
        }
    }
}

impl FromStr for Calendar {
    type Err = CalendarError;

    /// Parses multiline string into a vector of `RecurrentEvents`
    /// Using `rrule` library is not sufficient to capture duration of the event so we need to parse it
    /// manually ane remove it from the string before letting rrule parse the rest
    /// Duration has to be the last part of the RRULE_SET header after DTSTART e.g.
    ///   "DTSTART:20221020T180000Z;DURATION:PT1H" not "DURATION:PT1H;DTSTART:20221020T180000Z"
    /// Duration is in ISO8601 format (`iso8601_duration` crate)
    fn from_str(calendar_str: &str) -> Result<Self, Self::Err> {
        router_debug!("Parsing calendar: {}", calendar_str);
        let rrule_sets: Vec<&str> = calendar_str
            .split("DTSTART:")
            .filter(|s| !s.is_empty())
            .collect();
        router_debug!("rrule_sets: {:?}", rrule_sets);
        let mut recurrent_events: Vec<RecurrentEvent> = Vec::new();
        for rrule_set_str in rrule_sets {
            router_debug!("rrule_set_str: {}", rrule_set_str);
            let rrules_with_header: Vec<&str> = rrule_set_str
                .split('\n')
                .filter(|s| !s.is_empty())
                .collect();
            if rrules_with_header.len() < 2 {
                router_error!(
                    "Invalid rrule {} with header length: {}",
                    calendar_str,
                    rrules_with_header.len()
                );
                return Err(CalendarError::Rrule);
            }
            let header = rrules_with_header[0];
            let rrules = &rrules_with_header[1..];
            let header_parts: Vec<&str> = header
                .split(";DURATION:")
                .filter(|s| !s.is_empty())
                .collect();
            if header_parts.len() != 2 {
                router_error!("Invalid header parts length: {}", header_parts.len());
                return Err(CalendarError::HeaderPartsLength);
            }

            let dtstart = header_parts[0];
            let duration: &str = header_parts[1];
            let Ok(duration) = duration.parse::<Iso8601Duration>() else {
                router_error!("Invalid duration: {:?}", duration);
                return Err(CalendarError::Duration);
            };

            let Some(duration) = duration.to_chrono() else {
                router_error!("Could not convert duration to DateTime: {:?}", duration);
                return Err(CalendarError::Duration);
            };

            let str = "DTSTART:".to_owned() + dtstart + "\n" + rrules.join("\n").as_str();
            let rrset_res = RRuleSet::from_str(&str);

            let Ok(rrule_set) = rrset_res else {
                router_error!("Invalid rrule set: {:?}", rrset_res.unwrap_err());
                return Err(CalendarError::RruleSet);
            };

            recurrent_events.push(RecurrentEvent {
                rrule_set,
                duration,
            });
        }
        router_debug!("Parsed calendar: {:?}", recurrent_events);
        Ok(Calendar {
            events: recurrent_events,
        })
    }
}

impl Display for Calendar {
    /// Formats `Calendar` into multiline string which can be stored in the database
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        for event in &self.events {
            if let Err(e) = writeln!(
                f,
                "DTSTART:{};DURATION:{}",
                datetime_to_ical_format(event.rrule_set.get_dt_start()),
                &event.duration
            ) {
                router_error!("(Calendar fmt) {}", e);
                return Err(std::fmt::Error);
            }

            for rrule in event.rrule_set.get_rrule() {
                if let Err(e) = writeln!(f, "RRULE:{}", rrule) {
                    router_error!("(Calendar fmt) {}", e);
                    return Err(std::fmt::Error);
                }
            }

            for rdate in event.rrule_set.get_rdate() {
                if let Err(e) = writeln!(f, "RDATE:{}", datetime_to_ical_format(rdate)) {
                    router_error!("(Calendar fmt) {}", e);
                    return Err(std::fmt::Error);
                }
            }
        }

        Ok(())
    }
}

impl Calendar {
    /// Converts a date into a sorted list of timeslots for a given date
    pub fn to_timeslots(
        &self,
        time_start: &DateTime<Utc>,
        time_end: &DateTime<Utc>,
    ) -> Result<Vec<Timeslot>, CalendarError> {
        // Want to grab the full day's schedule, so we need to expand the start and end times
        let delta = Duration::try_days(1).ok_or_else(|| {
            router_error!("(Calendar to_timeslots) error creating time delta.");
            CalendarError::Internal
        })?;

        let start: NaiveDateTime = (*time_start).naive_utc() - delta;
        let end: NaiveDateTime = (*time_end).naive_utc() + delta;

        // convert to a Tz type understood by the rrule library
        let start: DateTime<rrule::Tz> = rrule::Tz::UTC.from_utc_datetime(&start);
        let end: DateTime<rrule::Tz> = rrule::Tz::UTC.from_utc_datetime(&end);

        let mut timeslots = vec![];
        for event in &self.events {
            let rrule = event.rrule_set.clone().after(start).before(end);
            for dt in rrule.all(MAX_EVENT_COUNT).dates {
                router_debug!(
                    "(Calendar to_timeslots) Found timeslot for event {:?}: {:?}",
                    event,
                    dt
                );

                let slot_start = dt.with_timezone(&Utc);
                let slot_end = slot_start + event.duration;
                if slot_start >= *time_end || slot_end <= *time_start {
                    continue;
                }

                let timeslot = Timeslot {
                    time_start: max(slot_start, *time_start),
                    time_end: min(slot_end, *time_end),
                };

                router_debug!("(Calendar to_timeslots) timeslot: {:?}", &timeslot);
                timeslots.push(timeslot);
            }
        }

        Ok(timeslots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lib_common::time::{Duration, LocalResult, TimeZone, Utc};
    use std::str::FromStr;

    const CAL_WORKDAYS_8AM_6PM: &str = "DTSTART:20221020T180000Z;DURATION:PT14H\n\
    RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\n\
    DTSTART:20221022T000000Z;DURATION:PT24H\n\
    RRULE:FREQ=DAILY;BYDAY=SA,SU";

    const _WITH_1HR_DAILY_BREAK: &str = "\n\
    DTSTART:20221020T120000Z;DURATION:PT1H\n\
    RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR";

    const _WITH_ONE_OFF_BLOCK: &str = "\n\
    DTSTART:20221026T133000Z;DURATION:PT3H\n\
    RDATE:20221026T133000Z";

    const INVALID_CALENDAR: &str = "DURATION:PT3H;DTSTART:20221026T133000Z;\n\
    RRULE:FREQ=DAILY;BYDAY=SA,SU";

    #[test]
    fn test_parse_calendar() {
        let calendar = Calendar::from_str(CAL_WORKDAYS_8AM_6PM).unwrap();
        assert_eq!(calendar.events.len(), 2);
        assert_eq!(
            calendar.events[0].duration,
            Duration::try_hours(14).unwrap()
        );
        assert_eq!(
            calendar.events[1].duration,
            Duration::try_hours(24).unwrap()
        );
    }

    #[test]
    fn test_save_and_load_calendar() {
        let orig_cal_str =
            &(CAL_WORKDAYS_8AM_6PM.to_owned() + _WITH_1HR_DAILY_BREAK + _WITH_ONE_OFF_BLOCK);
        let calendar = Calendar::from_str(orig_cal_str).unwrap();
        let cal_str = calendar.to_string();
        let calendar = Calendar::from_str(&cal_str).unwrap();
        assert_eq!(calendar.events.len(), 4);
        assert_eq!(
            calendar.events[0].duration,
            Duration::try_hours(14).unwrap()
        );
    }

    #[test]
    #[should_panic]
    fn test_invalid_input() {
        let _calendar = Calendar::from_str(INVALID_CALENDAR).unwrap();
    }

    #[test]
    fn test_calendar_to_timeslots() {
        // 8AM to 12PM, 2PM to 6PM
        let calendar = "DTSTART:20221020T080000Z;DURATION:PT4H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\n\
        DTSTART:20221020T140000Z;DURATION:PT4H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\n";

        let calendar = Calendar::from_str(calendar).unwrap();
        assert_eq!(calendar.events.len(), 2);

        let cal_start = Utc.with_ymd_and_hms(2022, 10, 20, 8, 0, 0).unwrap();
        let expected_timeslots = vec![
            Timeslot {
                time_start: cal_start,                                 // 8AM
                time_end: cal_start + Duration::try_hours(4).unwrap(), // 12PM
            },
            Timeslot {
                time_start: cal_start + Duration::try_hours(6).unwrap(), // 2PM
                time_end: cal_start + Duration::try_hours(10).unwrap(),  // 6PM
            },
        ];

        // Get full day schedule
        let timeslots = calendar
            .to_timeslots(
                &(cal_start - Duration::try_hours(1).unwrap()),
                &(cal_start + Duration::try_hours(12).unwrap()),
            )
            .unwrap();
        assert_eq!(timeslots.len(), 2);
        assert_eq!(timeslots, expected_timeslots);
    }

    #[test]
    fn test_calendar_to_timeslots_cropped() {
        // 8AM to 12PM, 2PM to 6PM
        let calendar = "DTSTART:20221020T080000Z;DURATION:PT4H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\n\
        DTSTART:20221020T140000Z;DURATION:PT4H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\n";

        let calendar = Calendar::from_str(calendar).unwrap();
        assert_eq!(calendar.events.len(), 2);

        let cal_start = Utc.with_ymd_and_hms(2022, 10, 20, 8, 0, 0).unwrap();

        // Crop to 10AM to 6PM
        let start: DateTime<Utc> = cal_start + Duration::try_hours(2).unwrap();
        let end: DateTime<Utc> = cal_start + Duration::try_hours(8).unwrap();

        let expected_timeslots = vec![
            Timeslot {
                time_start: start,                                     // 10 AM
                time_end: cal_start + Duration::try_hours(4).unwrap(), // 12PM
            },
            Timeslot {
                time_start: cal_start + Duration::try_hours(6).unwrap(), // 2PM
                time_end: end,                                           // 4PM
            },
        ];

        // Get full day schedule
        let timeslots = calendar.to_timeslots(&start, &end).unwrap();
        assert_eq!(timeslots.len(), 2);
        assert_eq!(timeslots, expected_timeslots);
    }

    #[test]
    fn test_calendar_to_timeslots_cropped_to_single() {
        // 8AM to 12PM, 2PM to 6PM
        let calendar = "DTSTART:20221020T080000Z;DURATION:PT4H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\n\
        DTSTART:20221020T140000Z;DURATION:PT4H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR\n";

        let calendar = Calendar::from_str(calendar).unwrap();
        assert_eq!(calendar.events.len(), 2);

        let cal_start = Utc.with_ymd_and_hms(2022, 10, 20, 8, 0, 0).unwrap();

        // Crop to 10AM to 6PM
        let start: DateTime<Utc> = cal_start + Duration::try_hours(2).unwrap();
        let end: DateTime<Utc> = cal_start + Duration::try_hours(3).unwrap();

        let expected_timeslots = vec![Timeslot {
            time_start: start, // 10 AM
            time_end: end,     // 11AM
        }];

        // Get full day schedule
        let timeslots = calendar.to_timeslots(&start, &end).unwrap();
        assert_eq!(timeslots.len(), 1);
        assert_eq!(timeslots, expected_timeslots);
    }

    #[test]
    fn test_calendar_to_timeslots_future() {
        // 8AM to 12PM, 2PM to 6PM
        let calendar = "DTSTART:20221020T080000Z;DURATION:PT4H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR,SA,SU\n\
        DTSTART:20221020T140000Z;DURATION:PT4H\n\
        RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR,SA,SU"
            .to_string();

        let calendar = Calendar::from_str(&calendar).unwrap();
        assert_eq!(calendar.events.len(), 2);

        let cal_start = Utc.with_ymd_and_hms(2023, 10, 20, 8, 0, 0).unwrap();

        // Crop to 10AM to 6PM
        let start: DateTime<Utc> = cal_start - Duration::try_hours(2).unwrap();
        let end: DateTime<Utc> = cal_start + Duration::try_hours(16).unwrap();

        let expected_timeslots = vec![
            Timeslot {
                time_start: cal_start,                                 // 8
                time_end: cal_start + Duration::try_hours(4).unwrap(), // 12
            },
            Timeslot {
                time_start: cal_start + Duration::try_hours(6).unwrap(), // 14
                time_end: cal_start + Duration::try_hours(10).unwrap(),  // 18
            },
        ];

        // Get full day schedule
        let timeslots = calendar.to_timeslots(&start, &end).unwrap();
        assert_eq!(timeslots.len(), 2);
        assert_eq!(timeslots, expected_timeslots);
    }

    /// |         a         |
    ///           -
    ///      |    b    |
    ///           =
    /// |    |         |    |
    #[test]
    fn test_timeslot_sub_cleave() {
        let LocalResult::Single(dt_start) = Utc.with_ymd_and_hms(2023, 10, 24, 0, 0, 0) else {
            panic!();
        };

        let timeslot_a = Timeslot {
            time_start: dt_start,
            time_end: dt_start + Duration::try_hours(3).unwrap(),
        };

        let timeslot_b = Timeslot {
            time_start: dt_start + Duration::try_hours(1).unwrap(),
            time_end: dt_start + Duration::try_hours(2).unwrap(),
        };

        let timeslots = timeslot_a - timeslot_b;
        assert_eq!(timeslots.len(), 2);
        assert_eq!(
            timeslots,
            vec![
                Timeslot {
                    time_start: dt_start,
                    time_end: dt_start + Duration::try_hours(1).unwrap(),
                },
                Timeslot {
                    time_start: dt_start + Duration::try_hours(2).unwrap(),
                    time_end: dt_start + Duration::try_hours(3).unwrap(),
                },
            ]
        );
    }

    /// |         a         |
    ///           -
    ///                |    b    |
    ///           =
    /// |              |
    #[test]
    fn test_timeslot_sub_crop_end() {
        let LocalResult::Single(dt_start) = Utc.with_ymd_and_hms(2023, 10, 24, 0, 0, 0) else {
            panic!();
        };

        let timeslot_a = Timeslot {
            time_start: dt_start,
            time_end: dt_start + Duration::try_hours(3).unwrap(),
        };

        let timeslot_b = Timeslot {
            time_start: dt_start + Duration::try_hours(2).unwrap(),
            time_end: dt_start + Duration::try_hours(4).unwrap(),
        };

        let timeslots = timeslot_a - timeslot_b;
        assert_eq!(timeslots.len(), 1);
        assert_eq!(
            timeslots,
            vec![Timeslot {
                time_start: dt_start,
                time_end: dt_start + Duration::try_hours(2).unwrap(),
            },]
        );
    }

    ///       |         a         |
    ///           -
    /// |    b    |
    ///           =
    ///           |               |
    #[test]
    fn test_timeslot_sub_crop_start() {
        let LocalResult::Single(dt_start) = Utc.with_ymd_and_hms(2023, 10, 24, 0, 0, 0) else {
            panic!();
        };

        let timeslot_a = Timeslot {
            time_start: dt_start + Duration::try_hours(1).unwrap(),
            time_end: dt_start + Duration::try_hours(3).unwrap(),
        };

        let timeslot_b = Timeslot {
            time_start: dt_start,
            time_end: dt_start + Duration::try_hours(2).unwrap(),
        };

        let timeslots = timeslot_a - timeslot_b;
        assert_eq!(timeslots.len(), 1);
        assert_eq!(
            timeslots,
            vec![Timeslot {
                time_start: dt_start + Duration::try_hours(2).unwrap(),
                time_end: dt_start + Duration::try_hours(3).unwrap(),
            },]
        );
    }

    #[test]
    fn test_timeslot_sub_no_overlap() {
        let LocalResult::Single(dt_start) = Utc.with_ymd_and_hms(2023, 10, 24, 0, 0, 0) else {
            panic!();
        };

        //           |         a         |
        //           -
        // |    b    |
        //           =
        //           |               |
        let timeslot_a = Timeslot {
            time_start: dt_start + Duration::try_hours(2).unwrap(),
            time_end: dt_start + Duration::try_hours(3).unwrap(),
        };

        let timeslot_b = Timeslot {
            time_start: dt_start,
            time_end: dt_start + Duration::try_hours(2).unwrap(),
        };

        let timeslots = timeslot_a - timeslot_b;
        assert_eq!(timeslots.len(), 1);
        assert_eq!(timeslots, vec![timeslot_a]);

        // |         a         |
        //           -
        //                     |    b    |
        //           =
        //           |               |
        let timeslot_a = Timeslot {
            time_start: dt_start,
            time_end: dt_start + Duration::try_hours(1).unwrap(),
        };

        let timeslot_b = Timeslot {
            time_start: dt_start + Duration::try_hours(1).unwrap(),
            time_end: dt_start + Duration::try_hours(2).unwrap(),
        };

        let timeslots = timeslot_a - timeslot_b;
        assert_eq!(timeslots.len(), 1);
        assert_eq!(timeslots, vec![timeslot_a]);
    }
}
