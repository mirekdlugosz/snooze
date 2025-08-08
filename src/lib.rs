use std::str::FromStr;
use std::time::Duration;

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
    args
        .iter()
        .try_fold(Duration::ZERO, |acc, arg| acc.checked_add(parse_pause_arg(arg)?))
        .and_then(|v| {
            if v.is_zero() {
                return None;
            }
            Some(v)
        })
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
}
