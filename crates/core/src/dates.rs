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

/// Days from 1 January 1970 to a date (Hinnant's days-from-civil), the inverse of `civil`.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A month or weekday number from its name in any of the interface's languages, whole or cut
/// to at least `shortest` letters (`MAR`, `março`, `SEG`), ignoring case.
fn by_name(word: &str, ids: &[&str], shortest: usize) -> Option<u32> {
    let word = word.to_uppercase();
    if word.chars().count() < shortest {
        return None;
    }
    for lang in mksigma_i18n::Lang::ALL {
        for id in ids {
            let list = mksigma_i18n::message_in(lang, id, &[]);
            if let Some(k) = list.split_whitespace().position(|n| n.to_uppercase() == word || n.to_uppercase().starts_with(&word)) {
                return u32::try_from(k + 1).ok();
            }
        }
    }
    None
}

fn month_of(word: &str) -> Option<u32> {
    match word.parse::<u32>() {
        Ok(m) if (1..=12).contains(&m) => Some(m),
        Ok(_) => None,
        Err(_) => by_name(word, &["date-months", "date-months-full"], 3),
    }
}

/// A year as typed: four digits as they are, two in the window 1950–2049.
fn year_of(word: &str) -> Option<i64> {
    let y: i64 = word.parse().ok()?;
    match word.len() {
        1 | 2 => Some(if y < 50 { 2000 + y } else { 1900 + y }),
        3..=4 => Some(y),
        _ => None,
    }
}

/// The day number of a valid date.
fn date(y: i64, m: u32, d: u32) -> Option<i64> {
    ((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)).then(|| days_from_civil(y, m, d) + TO_1970)
}

/// `h:mm[:ss[.ff]]` (or `mm:ss` when `minutes_first`) in seconds; hours may pass 24.
fn time_of(text: &str, minutes_first: bool) -> Option<f64> {
    let parts: Vec<&str> = text.split(':').collect();
    let number = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.0);
    let (h, m, s) = match (parts.as_slice(), minutes_first) {
        ([m, s], true) => (0.0, number(m)?, number(s)?),
        ([h, m], false) => (number(h)?, number(m)?, 0.0),
        ([h, m, s], _) => (number(h)?, number(m)?, number(s)?),
        _ => return None,
    };
    let whole = |x: f64| x.fract() == 0.0;
    (whole(h) && whole(m) && (minutes_first || m < 60.0) && s < 60.0).then_some(h * 3600.0 + m * 60.0 + s)
}

/// Typed text in a date or time format, as the number it stores: `Some(None)` when the text
/// isn't a date of that kind, `None` for formats that aren't dates. Day, month and year come
/// in the format's order (`05-MAR-2024`, `03/05/2024`, `2024/03/05`), or as `2024-03-05` in
/// any date format; months by number or by name in either language; a time after the date.
pub(crate) fn parse(kind: FormatKind, text: &str) -> Option<Option<f64>> {
    use FormatKind as K;
    let text = text.trim();
    let date_kind = matches!(kind, K::Date | K::ADate | K::EDate | K::SDate | K::JDate | K::QYr | K::MoYr | K::WkYr | K::DateTime | K::YmdHms);
    if !date_kind && !matches!(kind, K::Time | K::DTime | K::MTime | K::WkDay | K::Month) {
        return None;
    }
    let value = || -> Option<f64> {
        match kind {
            K::Time => return time_of(text, false),
            K::MTime => return time_of(text, true),
            K::DTime => {
                let (days, clock) = text.split_once(char::is_whitespace)?;
                let days: u32 = days.parse().ok()?;
                return Some(f64::from(days) * 86_400.0 + time_of(clock.trim(), false)?);
            }
            K::WkDay => {
                return text.parse::<u32>().ok().filter(|d| (1..=7).contains(d)).or_else(|| by_name(text, &["date-weekdays"], 2)).map(f64::from);
            }
            K::Month => return month_of(text).map(f64::from),
            _ => {}
        }
        // A time of day may follow the date, after a space.
        let (day_part, clock) = match text.split_once(char::is_whitespace) {
            Some((d, t)) if matches!(kind, K::DateTime | K::YmdHms) || t.contains(':') => (d, Some(t.trim())),
            _ => (text, None),
        };
        let seconds = match clock {
            Some(t) => time_of(t, false).filter(|s| *s < 86_400.0)?,
            None => 0.0,
        };
        let words: Vec<&str> = day_part.split(['-', '/', '.', ',', ' ']).filter(|w| !w.is_empty()).collect();
        let day = match (kind, words.as_slice()) {
            // ISO, in any date format.
            (_, [y, m, d]) if y.len() == 4 && y.parse::<i64>().is_ok() => date(y.parse().ok()?, month_of(m)?, d.parse().ok()?)?,
            (K::ADate, [m, d, y]) => date(year_of(y)?, month_of(m)?, d.parse().ok()?)?,
            (K::SDate | K::YmdHms, [y, m, d]) => date(year_of(y)?, month_of(m)?, d.parse().ok()?)?,
            (K::Date | K::EDate | K::DateTime, [d, m, y]) => date(year_of(y)?, month_of(m)?, d.parse().ok()?)?,
            (K::JDate, [yd]) if yd.len() == 7 || yd.len() == 5 => {
                let (y, d) = yd.split_at(yd.len() - 3);
                let (y, d): (i64, u32) = (year_of(y)?, d.parse().ok()?);
                let length = if days_in_month(y, 2) == 29 { 366 } else { 365 };
                ((1..=length).contains(&d)).then(|| days_from_civil(y, 1, 1) + TO_1970 + i64::from(d) - 1)?
            }
            (K::MoYr, [m, y]) => date(year_of(y)?, month_of(m)?, 1)?,
            (K::QYr, [q, letter, y]) if letter.eq_ignore_ascii_case("Q") => {
                let q: u32 = q.parse().ok().filter(|q| (1..=4).contains(q))?;
                date(year_of(y)?, 3 * q - 2, 1)?
            }
            (K::WkYr, [w, letters, y]) if letters.eq_ignore_ascii_case("WK") => {
                let w: i64 = w.parse().ok().filter(|w| (1..=53).contains(w))?;
                days_from_civil(year_of(y)?, 1, 1) + TO_1970 + 7 * (w - 1)
            }
            _ => return None,
        };
        Some(day as f64 * 86_400.0 + seconds)
    };
    Some(value())
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
    fn typed_dates_read_back_as_they_show() {
        let x = MARCH_5_2024 * 86_400.0;
        let cases = [
            (FormatKind::Date, "05-MAR-2024"),
            (FormatKind::Date, "5 march 2024"),
            (FormatKind::Date, "05-mar-24"),
            (FormatKind::Date, "2024-03-05"),
            (FormatKind::ADate, "03/05/2024"),
            (FormatKind::EDate, "05.03.2024"),
            (FormatKind::SDate, "2024/03/05"),
            (FormatKind::JDate, "2024065"),
        ];
        for (kind, text) in cases {
            assert_eq!(parse(kind, text), Some(Some(x)), "{kind:?} {text}");
        }
        assert_eq!(parse(FormatKind::DateTime, "05-MAR-2024 13:07:09.25"), Some(Some(x + 47_229.25)));
        assert_eq!(parse(FormatKind::YmdHms, "2024-03-05 13:07"), Some(Some(x + 47_220.0)));
        assert_eq!(parse(FormatKind::MoYr, "MAR 2024"), Some(Some((MARCH_5_2024 - 4.0) * 86_400.0)));
        assert_eq!(parse(FormatKind::QYr, "1 Q 2024"), Some(Some((MARCH_5_2024 - 64.0) * 86_400.0)));
        assert_eq!(parse(FormatKind::Time, "30:00:00"), Some(Some(108_000.0)));
        assert_eq!(parse(FormatKind::MTime, "02:05"), Some(Some(125.0)));
        assert_eq!(parse(FormatKind::DTime, "01 01:01:01"), Some(Some(90_061.0)));
        assert_eq!(parse(FormatKind::WkDay, "mon"), Some(Some(2.0)));
        assert_eq!(parse(FormatKind::Month, "dez"), Some(Some(12.0)), "Portuguese names too");
        // Not dates: impossible days, a bare number, other formats.
        for text in ["02/30/2024", "2024", "03-XYZ-2024", "13/05/2024", ""] {
            assert_eq!(parse(FormatKind::ADate, text), Some(None), "{text}");
        }
        assert_eq!(parse(FormatKind::Date, "29-FEB-2023"), Some(None));
        assert_eq!(parse(FormatKind::Time, "10:75"), Some(None));
        assert_eq!(parse(FormatKind::F, "05-MAR-2024"), None);
        // Every format reads back what it shows.
        for kind in
            [FormatKind::Date, FormatKind::ADate, FormatKind::EDate, FormatKind::SDate, FormatKind::JDate, FormatKind::DateTime, FormatKind::YmdHms]
        {
            let width = match kind {
                FormatKind::DateTime => 20,
                FormatKind::YmdHms => 19,
                FormatKind::JDate => 7,
                _ => 11,
            };
            let shown = show(kind, width, 0, x + 3_600.0 * f64::from(matches!(kind, FormatKind::DateTime | FormatKind::YmdHms)));
            assert!(parse(kind, &shown).flatten().is_some(), "{kind:?} {shown}");
        }
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
