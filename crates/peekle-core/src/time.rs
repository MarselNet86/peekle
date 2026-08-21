//! Time parsing shared by the parts of the product that read someone else's
//! timestamps. Pure, so the property tests can hammer it.

/// Seconds since the epoch for an ISO 8601 timestamp such as
/// `2026-08-20T16:30:00.366734+00:00`.
///
/// Hand rolled rather than pulled from a date crate: this is the only date the
/// product parses, the shape is fixed by the server, and a wrong answer here is
/// a wrong countdown rather than a crash. Fractional seconds are dropped and
/// the offset is applied.
pub fn iso_seconds(raw: &str) -> Option<i64> {
    let bytes = raw.as_bytes();
    if bytes.len() < 19 {
        return None;
    }

    let num = |from: usize, to: usize| raw.get(from..to)?.parse::<i64>().ok();
    let year = num(0, 4)?;
    let month = num(5, 7)?;
    let day = num(8, 10)?;
    let hour = num(11, 13)?;
    let minute = num(14, 16)?;
    let second = num(17, 19)?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    Some(seconds - offset_seconds(&raw[19..])?)
}

/// The trailing offset, in seconds east of UTC. `Z`, nothing at all, and
/// `+00:00` all mean the same thing.
fn offset_seconds(rest: &str) -> Option<i64> {
    let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let sign = match rest.as_bytes().first() {
        None | Some(b'Z' | b'z') => return Some(0),
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return None,
    };

    let hours = rest.get(1..3)?.parse::<i64>().ok()?;
    // `+0130` and `+01:30` are both legal, and both appear in the wild.
    let minutes = rest
        .get(3..)
        .map(|tail| tail.trim_start_matches(':'))
        .filter(|tail| !tail.is_empty())
        .map_or(Some(0), |tail| tail.get(0..2)?.parse::<i64>().ok())?;

    Some(sign * (hours * 3600 + minutes * 60))
}

/// Days from the epoch for a civil date. Howard Hinnant's algorithm, valid for
/// any proleptic Gregorian date, which is more than a reset timestamp needs.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
