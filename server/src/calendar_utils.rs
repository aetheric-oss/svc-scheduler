use chrono::{DateTime, Duration};
use chrono_tz::Tz as ChronoTz;
use iso8601_duration::Duration as DurationParser;
pub use rrule::{RRuleSet, Tz};
use std::fmt::Display;
use std::str::FromStr;

fn datetime_to_ical_format(dt: &DateTime<Tz>) -> String {
    let mut tz_prefix = String::new();
    let mut tz_postfix = String::new();
    let tz = dt.timezone();
    match tz {
        Tz::Local(_) => {}
        Tz::Tz(tz) => match tz {
            ChronoTz::UTC => {
                tz_postfix = "Z".to_string();
            }
            tz => {
                tz_prefix = format!(";TZID={}:", tz.name());
            }
        },
    }

    let dt = dt.format("%Y%m%dT%H%M%S");
    format!("{}{}{}", tz_prefix, dt, tz_postfix)
}

/// Wraps rruleset and their duration
#[derive(Debug)]
pub struct RecurrentEvent {
    /// The rruleset with recurrence rules
    pub rrule_set: RRuleSet,
    /// The duration of the event (iso8601 format)
    pub duration: String,
}
///Calendar implementation for recurring events using the rrule crate and duration iso8601_duration crate
#[derive(Debug)]
pub struct Calendar {
    ///Vec of rrulesets and their duration
    pub events: Vec<RecurrentEvent>,
}

impl FromStr for Calendar {
    type Err = ();

    /// Parses multiline string into a vector of `RecurrentEvents`
    /// Using `rrule` library is not sufficient to capture duration of the event so we need to parse it
    /// manually ane remove it from the string before letting rrule parse the rest
    /// Duration has to be the last part of the RRULE_SET header after DTSTART e.g.
    ///   "DTSTART:20221020T180000Z;DURATION:PT1H" not "DURATION:PT1H;DTSTART:20221020T180000Z"
    /// Duration is in ISO8601 format (`iso8601_duration` crate)
    fn from_str(calendar_str: &str) -> Result<Self, Self::Err> {
        let rrule_sets: Vec<&str> = calendar_str
            .split("DTSTART:")
            .filter(|s| !s.is_empty())
            .collect();
        let mut recurrent_events: Vec<RecurrentEvent> = Vec::new();
        for rrule_set_str in rrule_sets {
            let rrules_with_header: Vec<&str> = rrule_set_str
                .split('\n')
                .filter(|s| !s.is_empty())
                .collect();
            if rrules_with_header.len() < 2 {
                return Err(());
            }
            let header = rrules_with_header[0];
            let rrules = &rrules_with_header[1..];
            let header_parts: Vec<&str> = header
                .split(";DURATION:")
                .filter(|s| !s.is_empty())
                .collect();
            if header_parts.len() != 2 {
                return Err(());
            }
            let dtstart = header_parts[0];
            let duration = header_parts[1];
            let str = "DTSTART:".to_owned() + dtstart + "\n" + rrules.join("\n").as_str();
            let rrule_set = RRuleSet::from_str(&str).unwrap();
            recurrent_events.push(RecurrentEvent {
                rrule_set,
                duration: duration.to_string(),
            });
        }
        Ok(Calendar {
            events: recurrent_events,
        })
    }
}

impl Display for Calendar {
    /// Formats `Calendar` into multiline string which can be stored in the database
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let err_msg = String::from("Error writing to string");
        for event in &self.events {
            writeln!(
                f,
                "DTSTART:{};DURATION:{}",
                datetime_to_ical_format(event.rrule_set.get_dt_start()),
                &event.duration
            )
            .expect(&err_msg);
            for rrule in event.rrule_set.get_rrule() {
                writeln!(f, "RRULE:{}", rrule).expect(&err_msg);
            }
            for rdate in event.rrule_set.get_rdate() {
                writeln!(f, "RDATE:{}", datetime_to_ical_format(rdate)).expect(&err_msg);
            }
        }
        Ok(())
    }
}

impl Calendar {
    /// Wrapper implementation of rrule library's `all` method which also considers duration of the event
    pub fn is_available_between(&self, start_time: DateTime<Tz>, end_time: DateTime<Tz>) -> bool {
        // adjust start and end time by one second to make search inclusive of boundary values
        let start_time = start_time + Duration::seconds(1);
        let end_time = end_time - Duration::seconds(1);
        for event in &self.events {
            let duration = &event.duration;
            // check standard rrule time - if event(block) start time is between two dates,
            // then it will be found and time slot will be marked as not available
            let (events, _) = &event
                .rrule_set
                .clone()
                .after(start_time)
                .before(end_time)
                .all(1);
            if !events.is_empty() {
                return false;
            }
            let d = DurationParser::parse(duration).expect("Failed to parse duration");
            let adjusted_start_time = start_time
                - Duration::days(d.day as i64)
                - Duration::hours(d.hour as i64)
                - Duration::minutes(d.minute as i64)
                - Duration::seconds(d.second as i64);
            // here we check if event(block) start time + duration is between two dates,
            // then it will be found and time slot will be marked as not available
            let (events, _) = &event
                .rrule_set
                .clone()
                .after(adjusted_start_time)
                .before(end_time)
                .all(10);
            if !events.is_empty() {
                return false;
            }
        }
        // if no events(blocks) found across all rrule_sets, then time slot is available
        true
    }
}

#[cfg(test)]
mod calendar_tests {
    use super::{Calendar, RecurrentEvent};
    use chrono::TimeZone;
    use rrule::{RRuleSet, Tz};
    use std::str::FromStr;

    const CAL_WORKDAYS_8AM_6PM: &str = "DTSTART:20221020T180000Z;DURATION:PT14H\n\
    RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR\n\
    DTSTART:20221022T000000Z;DURATION:PT24H\n\
    RRULE:FREQ=WEEKLY;BYDAY=SA,SU";

    const _WITH_1HR_DAILY_BREAK: &str = "\n\
    DTSTART:20221020T120000Z;DURATION:PT1H\n\
    RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR";

    const _WITH_ONE_OFF_BLOCK: &str = "\n\
    DTSTART:20221026T133000Z;DURATION:PT3H\n\
    RDATE:20221026T133000Z";

    const INVALID_CALENDAR: &str = "DURATION:PT3H;DTSTART:20221026T133000Z;\n\
    RRULE:FREQ=WEEKLY;BYDAY=SA,SU";

    #[test]
    fn test_parse_calendar() {
        let calendar = Calendar::from_str(CAL_WORKDAYS_8AM_6PM).unwrap();
        assert_eq!(calendar.events.len(), 2);
        assert_eq!(calendar.events[0].duration, "PT14H");
    }

    #[test]
    fn test_night_unavailable() {
        let calendar = Calendar::from_str(CAL_WORKDAYS_8AM_6PM).unwrap();
        let start = Tz::UTC.ymd(2022, 10, 25).and_hms(19, 0, 0);
        let end = Tz::UTC.ymd(2022, 10, 25).and_hms(20, 0, 0);
        assert_eq!(calendar.is_available_between(start, end), false);
    }

    #[test]
    fn test_weekend_unavailable() {
        let calendar = Calendar::from_str(CAL_WORKDAYS_8AM_6PM).unwrap();
        let start = Tz::UTC.ymd(2022, 10, 22).and_hms(19, 0, 0);
        let end = Tz::UTC.ymd(2022, 10, 22).and_hms(20, 0, 0);
        assert_eq!(calendar.is_available_between(start, end), false);
    }

    #[test]
    fn test_inclusive_boundaries_available() {
        let calendar = Calendar::from_str(CAL_WORKDAYS_8AM_6PM).unwrap();

        let mut start = Tz::UTC.ymd(2022, 10, 25).and_hms(17, 0, 0);
        let mut end = Tz::UTC.ymd(2022, 10, 25).and_hms(18, 0, 0);
        assert_eq!(calendar.is_available_between(start, end), true);

        start = Tz::UTC.ymd(2022, 10, 25).and_hms(8, 0, 0);
        end = Tz::UTC.ymd(2022, 10, 25).and_hms(9, 0, 0);
        assert_eq!(calendar.is_available_between(start, end), true);
    }

    #[test]
    fn test_calendar_with_day_break() {
        let calendar =
            Calendar::from_str(&(CAL_WORKDAYS_8AM_6PM.to_owned() + _WITH_1HR_DAILY_BREAK)).unwrap();

        let mut start = Tz::UTC.ymd(2022, 10, 25).and_hms(11, 30, 0);
        let mut end = Tz::UTC.ymd(2022, 10, 25).and_hms(12, 30, 0);
        assert_eq!(calendar.is_available_between(start, end), false);

        start = Tz::UTC.ymd(2022, 10, 25).and_hms(8, 0, 0);
        end = Tz::UTC.ymd(2022, 10, 25).and_hms(12, 0, 0);
        assert_eq!(calendar.is_available_between(start, end), true);

        start = Tz::UTC.ymd(2022, 10, 25).and_hms(12, 15, 0);
        end = Tz::UTC.ymd(2022, 10, 25).and_hms(12, 30, 0);
        assert_eq!(calendar.is_available_between(start, end), false);

        start = Tz::UTC.ymd(2022, 10, 25).and_hms(12, 59, 0);
        end = Tz::UTC.ymd(2022, 10, 25).and_hms(13, 30, 0);
        assert_eq!(calendar.is_available_between(start, end), false);
    }

    #[test]
    fn test_calendar_with_one_off_block() {
        let calendar =
            Calendar::from_str(&(CAL_WORKDAYS_8AM_6PM.to_owned() + _WITH_ONE_OFF_BLOCK)).unwrap();

        let mut start = Tz::UTC.ymd(2022, 10, 26).and_hms(13, 30, 0);
        let mut end = Tz::UTC.ymd(2022, 10, 26).and_hms(14, 30, 0);
        assert_eq!(calendar.is_available_between(start, end), false);

        start = Tz::UTC.ymd(2022, 10, 27).and_hms(13, 30, 0);
        end = Tz::UTC.ymd(2022, 10, 27).and_hms(14, 30, 0);
        assert_eq!(calendar.is_available_between(start, end), true);

        start = Tz::UTC.ymd(2022, 10, 26).and_hms(11, 00, 0);
        end = Tz::UTC.ymd(2022, 10, 26).and_hms(13, 30, 0);
        assert_eq!(calendar.is_available_between(start, end), true);

        start = Tz::UTC.ymd(2022, 10, 26).and_hms(11, 00, 0);
        end = Tz::UTC.ymd(2022, 10, 26).and_hms(13, 30, 1);
        assert_eq!(calendar.is_available_between(start, end), false);
    }

    #[test]
    fn test_save_and_load_calendar() {
        let orig_cal_str =
            &(CAL_WORKDAYS_8AM_6PM.to_owned() + _WITH_1HR_DAILY_BREAK + _WITH_ONE_OFF_BLOCK);
        let calendar = Calendar::from_str(orig_cal_str).unwrap();
        let cal_str = calendar.to_string();
        let calendar = Calendar::from_str(&cal_str).unwrap();
        assert_eq!(calendar.events.len(), 4);
        assert_eq!(calendar.events[0].duration, "PT14H");
    }

    #[test]
    #[should_panic]
    fn test_invalid_input() {
        let calendar = Calendar::from_str(INVALID_CALENDAR).unwrap();
    }
}
