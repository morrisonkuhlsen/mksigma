//! Dates and times as SPSS's date formats write them. A date is a number of seconds from
//! 14 October 1582 (the first day of the Gregorian calendar); a time, seconds; `WKDAY` and
//! `MONTH`, a weekday (1 = Sunday) or month number. The width decides between two- and
//! four-digit years and whether seconds show; the decimals, fractions of a second.

use mksigma_i18n::tr;

use crate::FormatKind;

/// Days from 14 October 1582 to 1 January 1970.
const TO_1970: i64 = 141_428;

/// The date of day `days` from 1 January 1970 (Hinnant's civil-from-days): year, month, day.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// Day of the year, 1 to 366.
fn day_of_year(year: i64, month: u32, day: u32) -> u32 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let past = before.get(month.saturating_sub(1) as usize).copied().unwrap_or(0);
    past + day + u32::from(leap && month > 2)
}

/// The `n`th (1-based) of a space-separated catalog list, or `n` itself.
fn name(list: &str, n: u32) -> String {
    list.split_whitespace().nth(n.saturating_sub(1) as usize).map_or_else(|| n.to_string(), str::to_string)
}

fn month_abbrev(m: u32) -> String {
    name(&tr!("date-months"), m)
}

/// The year in two digits when the width leaves no room for four.
fn year(y: i64, short: bool) -> String {
    if short { format!("{:02}", y.rem_euclid(100)) } else { format!("{y:04}") }
}

/// `hh:mm`, then `:ss` and its decimals as the width and decimals allow; `seconds` from the
/// day's start (or the time's, which may pass 24 hours).
fn clock(seconds: f64, show_seconds: bool, decimals: u8, pad_hours: bool) -> String {
    let h = (seconds / 3600.0).floor();
    let m = ((seconds - h * 3600.0) / 60.0).floor();
    let s = seconds - h * 3600.0 - m * 60.0;
    let hours = if pad_hours { format!("{h:02}") } else { format!("{h}") };
    if !show_seconds {
        return format!("{hours}:{m:02}");
    }
    if decimals == 0 {
        format!("{hours}:{m:02}:{:02}", s.floor())
    } else {
        let d = usize::from(decimals);
        format!("{hours}:{m:02}:{s:0w$.d$}", w = d + 3)
    }
}

/// `x` in a date or time format, or `None` for other formats and values no calendar holds.
pub(crate) fn render(kind: FormatKind, width: u16, decimals: u8, x: f64) -> Option<String> {
    use FormatKind as K;
    if !matches!(
        kind,
        K::Date
            | K::ADate
            | K::EDate
            | K::SDate
            | K::JDate
            | K::QYr
            | K::MoYr
            | K::WkYr
            | K::DateTime
            | K::YmdHms
            | K::Time
            | K::DTime
            | K::MTime
            | K::WkDay
            | K::Month
    ) || !x.is_finite()
        || x.abs() > 1e15
    {
        return None;
    }
    // Fractions of a second round to the decimals shown, before anything is split off.
    let scale = 10f64.powi(i32::from(decimals.min(6)));
    let x = (x * scale).round() / scale;
    let w = usize::from(width);
    match kind {
        K::WkDay => {
            let n = (x.round() as i64).clamp(1, 7) as u32;
            let full = name(&tr!("date-weekdays"), n);
            return Some(full.chars().take(w.max(2)).collect());
        }
        K::Month => {
            let n = (x.round() as i64).clamp(1, 12) as u32;
            let full = name(&tr!("date-months-full"), n);
            return Some(full.chars().take(w.max(3)).collect());
        }
        K::Time | K::MTime | K::DTime => {
            let negative = x < 0.0;
            let x = x.abs();
            let text = match kind {
                K::MTime => {
                    let m = (x / 60.0).floor();
                    let s = x - m * 60.0;
                    if decimals == 0 {
                        format!("{m:02}:{:02}", s.floor())
                    } else {
                        format!("{m:02}:{s:0w$.d$}", w = usize::from(decimals) + 3, d = usize::from(decimals))
                    }
                }
                K::DTime => {
                    let days = (x / 86_400.0).floor();
                    format!("{days:02} {}", clock(x - days * 86_400.0, w >= 11, decimals, true))
                }
                _ => clock(x, w >= 8, decimals, true),
            };
            return Some(if negative { format!("-{text}") } else { text });
        }
        _ => {}
    }
    let days = (x / 86_400.0).floor();
    let (y, m, d) = civil(days as i64 - TO_1970);
    let seconds = x - days * 86_400.0;
    Some(match kind {
        K::Date => format!("{d:02}-{}-{}", month_abbrev(m), year(y, w < 11)),
        K::ADate => format!("{m:02}/{d:02}/{}", year(y, w < 10)),
        K::EDate => format!("{d:02}.{m:02}.{}", year(y, w < 10)),
        K::SDate => format!("{}/{m:02}/{d:02}", year(y, w < 10)),
        K::JDate => format!("{}{:03}", year(y, w < 7), day_of_year(y, m, d)),
        K::QYr => format!("{} Q {}", (m - 1) / 3 + 1, year(y, w < 8)),
        K::MoYr => format!("{} {}", month_abbrev(m), year(y, w < 8)),
        K::WkYr => format!("{:02} WK {}", (day_of_year(y, m, d) - 1) / 7 + 1, year(y, w < 10)),
        K::DateTime => format!("{d:02}-{}-{y:04} {}", month_abbrev(m), clock(seconds, w >= 20, decimals, true)),
        K::YmdHms => format!("{y:04}-{m:02}-{d:02} {}", clock(seconds, w >= 19, decimals, true)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Days from 14 October 1582, counted independently (Python's `datetime`).
    const MARCH_5_2024: f64 = 161_215.0;
    const FEBRUARY_9_2024: f64 = 161_190.0;

    fn show(kind: FormatKind, width: u16, decimals: u8, x: f64) -> String {
        render(kind, width, decimals, x).unwrap()
    }

    #[test]
    fn day_zero_is_14_october_1582() {
        assert_eq!(show(FormatKind::Date, 11, 0, 0.0), "14-OCT-1582");
        assert_eq!(civil(-TO_1970), (1582, 10, 14));
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(11_016), (2000, 2, 29), "a leap day");
    }

    #[test]
    fn every_date_format() {
        // 5 March 2024, 13:07:09.25.
        let x = MARCH_5_2024 * 86_400.0 + 13.0 * 3600.0 + 7.0 * 60.0 + 9.25;
        let cases = [
            (FormatKind::Date, 11, 0, "05-MAR-2024"),
            (FormatKind::Date, 9, 0, "05-MAR-24"),
            (FormatKind::ADate, 10, 0, "03/05/2024"),
            (FormatKind::EDate, 10, 0, "05.03.2024"),
            (FormatKind::SDate, 10, 0, "2024/03/05"),
            (FormatKind::JDate, 7, 0, "2024065"),
            (FormatKind::QYr, 8, 0, "1 Q 2024"),
            (FormatKind::MoYr, 8, 0, "MAR 2024"),
            (FormatKind::WkYr, 10, 0, "10 WK 2024"),
            (FormatKind::DateTime, 20, 0, "05-MAR-2024 13:07:09"),
            (FormatKind::DateTime, 23, 2, "05-MAR-2024 13:07:09.25"),
            (FormatKind::DateTime, 17, 0, "05-MAR-2024 13:07"),
            (FormatKind::YmdHms, 19, 0, "2024-03-05 13:07:09"),
        ];
        for (kind, width, decimals, expected) in cases {
            assert_eq!(show(kind, width, decimals, x), expected, "{kind:?}{width}.{decimals}");
        }
    }

    #[test]
    fn times_weekdays_and_months() {
        assert_eq!(show(FormatKind::Time, 8, 0, 3661.0), "01:01:01");
        assert_eq!(show(FormatKind::Time, 5, 0, 3661.0), "01:01");
        assert_eq!(show(FormatKind::Time, 8, 0, 30.0 * 3600.0), "30:00:00", "a time may pass a day");
        assert_eq!(show(FormatKind::Time, 11, 2, 59.999), "00:01:00.00", "rounded before it is split");
        assert_eq!(show(FormatKind::MTime, 5, 0, 125.0), "02:05");
        assert_eq!(show(FormatKind::DTime, 11, 0, 90_061.0), "01 01:01:01");
        assert_eq!(show(FormatKind::WkDay, 9, 0, 2.0), "MONDAY");
        assert_eq!(show(FormatKind::WkDay, 3, 0, 2.0), "MON");
        assert_eq!(show(FormatKind::Month, 3, 0, 12.0), "DEC");
        assert_eq!(render(FormatKind::F, 8, 2, 1.0), None);
        assert_eq!(render(FormatKind::Date, 11, 0, f64::INFINITY), None);
        assert_eq!(render(FormatKind::Date, 11, 0, 1e300), None, "no calendar holds it");
    }

    #[test]
    fn names_follow_the_language() {
        mksigma_i18n::set(mksigma_i18n::Lang::from_code("pt-BR").unwrap());
        let feb = FEBRUARY_9_2024 * 86_400.0;
        let text = (show(FormatKind::Date, 11, 0, feb), show(FormatKind::WkDay, 9, 0, 7.0));
        mksigma_i18n::set(mksigma_i18n::Lang::from_code("en").unwrap());
        assert_eq!(text, ("09-FEV-2024".to_string(), "SÁBADO".to_string()));
    }
}
