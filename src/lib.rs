use std::fmt::Display;
use std::str::FromStr;
use std::time::Duration;

use time::OffsetDateTime;
use time::macros::format_description;

const MULTIPLIER_SECONDS: f64 = 1.0;
const MULTIPLIER_MINUTES: f64 = 60.0;
const MULTIPLIER_HOURS: f64 = 60.0 * 60.0;
const MULTIPLIER_DAYS: f64 = 24.0 * 60.0 * 60.0;

#[derive(Debug, PartialEq)]
enum SnoozeUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
}

#[derive(Debug, PartialEq, Eq)]
struct SnoozeUnitError;

impl FromStr for SnoozeUnit {
    type Err = SnoozeUnitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "s" => Ok(Self::Seconds),
            "m" => Ok(Self::Minutes),
            "h" => Ok(Self::Hours),
            "d" => Ok(Self::Days),
            _ => Err(SnoozeUnitError),
        }
    }
}

struct RemainingTime {
    seconds: u64,
    minutes: u64,
    hours: u64,
}

impl Display for RemainingTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hours = match self.hours {
            1.. => format!("{:3}:", self.hours),
            0 => "    ".to_string(),
        };
        let minutes = if self.hours > 0 {
            format!("{:02}:", self.minutes)
        } else {
            match self.minutes {
                10.. => format!("{}:", self.minutes),
                1..10 => format!("{:2}:", self.minutes),
                0 => "   ".to_string(),
            }
        };
        let seconds = if self.hours > 0 || self.minutes > 0 {
            format!("{:02}", self.seconds)
        } else {
            format!("{:2}", self.seconds)
        };

        write!(f, "{hours}{minutes}{seconds}")
    }
}

fn split_unit(input: &str) -> Option<(f64, SnoozeUnit)> {
    let (unit_char_pos, unit_char) = input.char_indices().last()?;
    let (str_num, str_unit) = if unit_char.is_alphabetic() {
        input.split_at(unit_char_pos)
    } else {
        (input, "s")
    };
    let num: f64 = str_num.parse().ok()?;
    let unit: SnoozeUnit = str_unit.parse().ok()?;
    Some((num, unit))
}

fn parse_pause_arg(input: &str) -> Option<Duration> {
    let input = input.trim();
    if input.is_empty() {
        return Some(Duration::ZERO);
    }

    let (number, unit) = split_unit(input)?;
    let multiplier = match unit {
        SnoozeUnit::Seconds => MULTIPLIER_SECONDS,
        SnoozeUnit::Minutes => MULTIPLIER_MINUTES,
        SnoozeUnit::Hours => MULTIPLIER_HOURS,
        SnoozeUnit::Days => MULTIPLIER_DAYS,
    };

    let seconds = number * multiplier;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let nano_seconds = (seconds * 1_000_000_000.0).trunc() as u64;

    Some(Duration::from_nanos(nano_seconds))
}

#[allow(clippy::must_use_candidate)]
pub fn sum_pause_args(args: &[&str]) -> Option<Duration> {
    args.iter()
        .try_fold(Duration::ZERO, |acc, arg| {
            acc.checked_add(parse_pause_arg(arg)?)
        })
        .and_then(|v| {
            if v.is_zero() {
                return None;
            }
            Some(v)
        })
}

fn calc_wall_clock_end_time(
    beginning: OffsetDateTime,
    duration: Duration,
) -> Option<OffsetDateTime> {
    let seconds = i64::try_from(duration.as_secs()).ok()?;
    let nanos = i32::try_from(duration.subsec_nanos()).ok()?;
    let time_duration = time::Duration::new(seconds, nanos);
    Some(beginning.saturating_add(time_duration))
}

fn format_wall_clock_end_time(beginning: OffsetDateTime, end: OffsetDateTime) -> Option<String> {
    let date = if beginning.date() == end.date() {
        String::new()
    } else {
        end.format(format_description!(version = 2, "[year]-[month]-[day] "))
            .ok()?
    };
    let time = end
        .format(format_description!(version = 2, "[hour]:[minute]:[second]"))
        .ok()?;
    Some(format!("{date}{time}"))
}

#[allow(clippy::must_use_candidate)]
pub fn wall_clock_end_time(input: Duration) -> Option<String> {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    let end = calc_wall_clock_end_time(now, input)?;
    format_wall_clock_end_time(now, end)
}

#[allow(clippy::must_use_candidate)]
pub fn format_remaining_time(input: Duration) -> String {
    let mut total_seconds = input.as_secs();
    if input.subsec_nanos() > 500_000_000 {
        total_seconds = total_seconds.saturating_add(1);
    }
    let hours = total_seconds.div_euclid(60 * 60);
    let remaining_minutes = total_seconds.rem_euclid(60 * 60);
    let minutes = remaining_minutes.div_euclid(60);
    let seconds = remaining_minutes.rem_euclid(60);

    let remaining = RemainingTime {
        seconds,
        minutes,
        hours,
    };
    remaining.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("", Duration::ZERO)]
    #[case(" ", Duration::ZERO)]
    #[case("1", Duration::from_secs(1))]
    #[case("1s", Duration::from_secs(1))]
    #[case("0.5", Duration::from_millis(500))]
    #[case("0.5s", Duration::from_millis(500))]
    #[case("2m", Duration::from_secs(2 * 60))]
    #[case("3h", Duration::from_secs(3 * 60 * 60))]
    #[case("4d", Duration::from_secs(4 * 60 * 60 * 24))]
    #[case("0.5m", Duration::from_secs(30))]
    #[case("0.25m", Duration::from_secs(15))]
    #[case("0.125m", Duration::from_millis(7500))]
    #[case("0.5d", Duration::from_secs(12 * 60 * 60))]
    #[case(" 1", Duration::from_secs(1))]
    #[case(" 1\t\n", Duration::from_secs(1))]
    fn test_parse_pause_arg_ok(#[case] input: &str, #[case] expected: Duration) {
        let result = parse_pause_arg(input);
        assert_eq!(result, Some(expected));
    }

    #[rstest]
    #[case("0 5")]
    #[case("s")]
    #[case("1m2d")]
    #[case("1m2")]
    #[case("1y")]
    #[case("1ms")]
    fn test_parse_pause_arg_invalid(#[case] input: &str) {
        let result = parse_pause_arg(input);
        assert_eq!(result, None)
    }

    #[test]
    fn test_sum_pause_args_empty() {
        let input = [];
        assert_eq!(None, sum_pause_args(&input));
    }

    #[test]
    fn test_sum_pause_args_ok() {
        let input = ["1s", "5s", "1m"];
        let expected = Duration::from_secs(1 + 5 + 60);
        assert_eq!(Some(expected), sum_pause_args(&input));
    }

    #[test]
    fn test_sum_pause_args_invalid() {
        let input = ["1s", "5y", "1m"];
        assert_eq!(None, sum_pause_args(&input));
    }

    #[rstest]
    #[case(1565442000, 3600)]
    #[case(1709208000, 3600)] // leap year
    #[case(1740744000, 3600)] // February non-leap year
    #[case(1745539140, 3600)] // cross midnight line - day
    #[case(1746057540, 3600)] // cross midnight line - month
    #[case(1735689540, 3600)] // cross midnight line - year
    #[case(1754690400, 36 * 60 * 60)] // more than a day duration
    fn test_calc_wall_clock_end_time(#[case] beginning_ts: i64, #[case] duration: i64) {
        let beginning = OffsetDateTime::from_unix_timestamp(beginning_ts).unwrap();
        let result = calc_wall_clock_end_time(beginning, Duration::from_secs(duration as u64));
        let expected = OffsetDateTime::from_unix_timestamp(beginning_ts + duration).unwrap();
        assert_eq!(result, Some(expected));
    }

    #[rstest]
    #[case(1565442000, 3600, "14:00:00")]
    #[case(1709208000, 3600, "13:00:00")] // leap year
    #[case(1740744000, 3600, "13:00:00")] // February non-leap year
    #[case(1745539140, 3600, "2025-04-25 00:59:00")] // cross midnight line - day
    #[case(1746057540, 3600, "2025-05-01 00:59:00")] // cross midnight line - month
    #[case(1735689540, 3600, "2025-01-01 00:59:00")] // cross midnight line - year
    #[case(1754690400, 36 * 60 * 60, "2025-08-10 10:00:00")] // more than a day duration
    fn test_format_wall_clock_end_time(
        #[case] beginning_ts: i64,
        #[case] duration: i64,
        #[case] expected: &str,
    ) {
        let beginning = OffsetDateTime::from_unix_timestamp(beginning_ts).unwrap();
        let end = OffsetDateTime::from_unix_timestamp(beginning_ts + duration).unwrap();
        let result = format_wall_clock_end_time(beginning, end);
        assert_eq!(result, Some(expected.to_string()));
    }

    #[rstest]
    #[case(Duration::from_secs(1), "        1")]
    #[case(Duration::from_secs(11), "       11")]
    #[case(Duration::from_secs(61), "     1:01")]
    #[case(Duration::from_secs(81), "     1:21")]
    #[case(Duration::from_secs(661), "    11:01")]
    #[case(Duration::from_secs(701), "    11:41")]
    #[case(Duration::from_secs(7200), "  2:00:00")]
    #[case(Duration::from_secs(7100), "  1:58:20")]
    #[case(Duration::from_secs(604800), "168:00:00")]
    #[case(Duration::from_millis(900), "        1")]
    #[case(Duration::from_millis(300), "        0")]
    fn test_format_remaining_time(#[case] input: Duration, #[case] expected: &str) {
        let result = format_remaining_time(input);
        assert_eq!(result, expected);
    }
}
