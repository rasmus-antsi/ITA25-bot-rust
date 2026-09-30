use chrono::{DateTime, Datelike, Days, NaiveDate, Utc, Weekday};
use chrono_tz::Europe::Tallinn;

pub fn date_in_tallinn(now: DateTime<Utc>) -> NaiveDate {
    now.with_timezone(&Tallinn).date_naive()
}

pub fn today_in_tallinn() -> NaiveDate {
    date_in_tallinn(Utc::now())
}

/// The Monday of the week that contains `date`.
pub fn monday_of(date: NaiveDate) -> NaiveDate {
    date - Days::new(u64::from(date.weekday().num_days_from_monday()))
}

/// The next day school can happen: Friday and the weekend roll over to Monday.
pub fn next_school_day(date: NaiveDate) -> NaiveDate {
    let step = match date.weekday() {
        Weekday::Fri => 3,
        Weekday::Sat => 2,
        _ => 1,
    };
    date + Days::new(step)
}

/// Understands `30.09.2026`, `30.09.26`, `2026-09-30` and `30.09` (this year).
pub fn parse_date(input: &str, today: NaiveDate) -> Option<NaiveDate> {
    let input = input.trim();
    ["%d.%m.%y", "%d.%m.%Y", "%Y-%m-%d"]
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(input, format).ok())
        .or_else(|| {
            NaiveDate::parse_from_str(&format!("{input}.{}", today.year()), "%d.%m.%Y").ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn monday_of_any_weekday() {
        assert_eq!(monday_of(d(2026, 9, 28)), d(2026, 9, 28)); // Monday
        assert_eq!(monday_of(d(2026, 9, 30)), d(2026, 9, 28)); // Wednesday
        assert_eq!(monday_of(d(2026, 10, 4)), d(2026, 9, 28)); // Sunday
    }

    #[test]
    fn next_school_day_skips_the_weekend() {
        assert_eq!(next_school_day(d(2026, 9, 28)), d(2026, 9, 29)); // Mon -> Tue
        assert_eq!(next_school_day(d(2026, 10, 1)), d(2026, 10, 2)); // Thu -> Fri
        assert_eq!(next_school_day(d(2026, 10, 2)), d(2026, 10, 5)); // Fri -> Mon
        assert_eq!(next_school_day(d(2026, 10, 3)), d(2026, 10, 5)); // Sat -> Mon
        assert_eq!(next_school_day(d(2026, 10, 4)), d(2026, 10, 5)); // Sun -> Mon
    }

    #[test]
    fn next_school_day_crosses_month_and_year() {
        assert_eq!(next_school_day(d(2026, 10, 30)), d(2026, 11, 2));
        assert_eq!(next_school_day(d(2026, 12, 31)), d(2027, 1, 1));
    }

    #[test]
    fn parse_date_accepts_common_formats() {
        let today = d(2026, 9, 30);
        assert_eq!(parse_date("30.09.2026", today), Some(d(2026, 9, 30)));
        assert_eq!(parse_date("30.09.26", today), Some(d(2026, 9, 30)));
        assert_eq!(parse_date("2026-09-30", today), Some(d(2026, 9, 30)));
        assert_eq!(parse_date("  30.09.2026 ", today), Some(d(2026, 9, 30)));
    }

    #[test]
    fn parse_date_without_year_uses_the_current_one() {
        let today = d(2026, 9, 30);
        assert_eq!(parse_date("2.10", today), Some(d(2026, 10, 2)));
        assert_eq!(parse_date("02.10", today), Some(d(2026, 10, 2)));
    }

    #[test]
    fn parse_date_rejects_nonsense() {
        let today = d(2026, 9, 30);
        assert_eq!(parse_date("31.02.2026", today), None);
        assert_eq!(parse_date("hello", today), None);
        assert_eq!(parse_date("", today), None);
    }

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn tallinn_date_is_ahead_of_utc_in_the_evening() {
        // Summer time (UTC+3): 21:30 UTC is already 00:30 the next day.
        assert_eq!(date_in_tallinn(utc("2026-09-30T21:30:00Z")), d(2026, 10, 1));
        assert_eq!(date_in_tallinn(utc("2026-09-30T20:30:00Z")), d(2026, 9, 30));
        // Winter time (UTC+2): 22:30 UTC is 00:30 the next day.
        assert_eq!(date_in_tallinn(utc("2026-12-31T22:30:00Z")), d(2027, 1, 1));
        assert_eq!(
            date_in_tallinn(utc("2026-12-31T21:30:00Z")),
            d(2026, 12, 31)
        );
    }
}
