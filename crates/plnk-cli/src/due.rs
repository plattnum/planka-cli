//! ISO 8601 parsing and strict due-date range matching for `card find`.
//!
//! Planka stores `dueDate` as an ISO 8601 string and offers no server-side
//! filter by due date, so filtering is a client-side predicate applied after
//! the candidate set is fetched. Comparison is done on the underlying UTC
//! instant; comparing the raw strings would misorder date-only values,
//! fractional seconds, and non-UTC offsets.

use plnk_core::error::PlankaError;
use plnk_core::models::{Card, DueDateUpdate};
use time::OffsetDateTime;
use time::UtcOffset;
use time::format_description::well_known::Rfc3339;

/// A strictly-open due-date interval: `after < due < before`.
///
/// An unset bound is unbounded on that side. A range with both bounds unset
/// matches every card, including cards without a due date, so the predicate
/// can be applied unconditionally.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DueDateRange {
    after: Option<OffsetDateTime>,
    before: Option<OffsetDateTime>,
}

impl DueDateRange {
    /// Build a range from optional raw bound expressions.
    ///
    /// `after_field` and `before_field` name the originating CLI flags so a
    /// parse failure maps to the right `InvalidOptionValue` field. `now` is the
    /// reference instant for relative expressions; callers derive it from the
    /// environment, tests pass a frozen value.
    pub fn parse(
        after: Option<&str>,
        after_field: &str,
        before: Option<&str>,
        before_field: &str,
        now: OffsetDateTime,
    ) -> Result<Self, PlankaError> {
        Ok(Self {
            after: after
                .map(|raw| parse_bound(raw, after_field, now))
                .transpose()?,
            before: before
                .map(|raw| parse_bound(raw, before_field, now))
                .transpose()?,
        })
    }

    /// Keep only cards whose due date falls strictly inside the range.
    ///
    /// Cards without a due date, or with an unparseable one, never match a
    /// bounded range. A bounded range also drops closed cards unless
    /// `include_closed` is set: asking what is due is a planning question, and
    /// a closed card is done. An unbounded range keeps everything and ignores
    /// `include_closed` entirely, so a bare listing is unchanged.
    #[must_use]
    pub fn filter(&self, mut cards: Vec<Card>, include_closed: bool) -> Vec<Card> {
        if self.after.is_none() && self.before.is_none() {
            return cards;
        }
        cards.retain(|card| self.matches(card, include_closed));
        cards
    }

    fn matches(&self, card: &Card, include_closed: bool) -> bool {
        if card.is_closed && !include_closed {
            return false;
        }
        let Some(raw) = card.due_date.as_deref() else {
            return false;
        };
        let due = match parse_iso8601_utc(raw) {
            Ok(due) => due,
            Err(reason) => {
                eprintln!(
                    "Warning: card {} has an unparseable dueDate '{raw}': {reason}; \
                     excluded from due-date filtering",
                    card.id
                );
                return false;
            }
        };
        let due = due.unix_timestamp_nanos();
        self.after
            .is_none_or(|after| after.unix_timestamp_nanos() < due)
            && self
                .before
                .is_none_or(|before| due < before.unix_timestamp_nanos())
    }
}

/// Grammar summary shown in every due-date parse error.
const DUE_DATE_GRAMMAR: &str = "expected an ISO 8601 timestamp (YYYY-MM-DD or \
     YYYY-MM-DDThh:mm[:ss[.fff]][Z|±hh:mm]) or a relative date such as 'now', \
     'today', 'tomorrow', 'next week', 'in 3 days', or '2 weeks from today'";

/// Resolve a due-date expression to an instant.
///
/// Relative forms are tried first, then ISO 8601. `now` is the reference
/// instant for relative expressions and supplies the offset used to derive the
/// calendar day.
fn parse_bound(raw: &str, field: &str, now: OffsetDateTime) -> Result<OffsetDateTime, PlankaError> {
    if let Some(relative) = parse_relative(raw, now) {
        return Ok(relative);
    }
    parse_iso8601_utc(raw).map_err(|reason| PlankaError::InvalidOptionValue {
        field: field.to_string(),
        message: format!("'{raw}' is not a valid due date: {DUE_DATE_GRAMMAR} ({reason})"),
    })
}

/// Build the reference instant for relative due dates.
///
/// Prefers the system's local offset, which `time` reads from `TZ` /
/// `/etc/localtime`. Falls back to UTC when the offset cannot be determined.
///
/// `time` caches the offset lookup in a thread-local and documents the call as
/// unsound if another thread mutates `TZ` concurrently; `plnk` calls this once,
/// at parse time, before meaningful threading starts.
#[must_use]
pub fn reference_now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

/// Start of the calendar day containing `now`, in `now`'s offset.
fn start_of_day(now: OffsetDateTime) -> OffsetDateTime {
    now.replace_time(time::Time::MIDNIGHT)
}

/// Parse a relative date expression against `now`.
///
/// Returns `None` for anything the grammar does not recognise, leaving ISO 8601
/// parsing to the caller. Bare relative dates resolve to the start of the day in
/// `now`'s offset; `now` is the exception and returns the reference instant
/// itself, keeping the time of day.
fn parse_relative(raw: &str, now: OffsetDateTime) -> Option<OffsetDateTime> {
    let text = raw.trim().to_ascii_lowercase();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let today = start_of_day(now);

    match text.as_str() {
        "now" => return Some(now),
        "today" => return Some(today),
        "tomorrow" => return Some(today + time::Duration::days(1)),
        "yesterday" => return Some(today - time::Duration::days(1)),
        _ => {}
    }

    if let Some(weekday) = parse_weekday(&text) {
        let today_weekday = today.weekday().number_days_from_monday();
        let target = weekday.number_days_from_monday();
        // Always the *next* occurrence, never today itself: today would make
        // `--due-before monday` behave inconsistently depending on the day.
        let delta = match (target + 7 - today_weekday) % 7 {
            0 => 7,
            days => i64::from(days),
        };
        return Some(today + time::Duration::days(delta));
    }

    if let Some(unit) = text.strip_prefix("this ").and_then(parse_unit) {
        return Some(start_of_unit(today, unit));
    }
    if let Some(unit) = text.strip_prefix("next ").and_then(parse_unit) {
        return Some(shift_units(start_of_unit(today, unit), unit, 1));
    }
    if let Some(unit) = text.strip_prefix("last ").and_then(parse_unit) {
        return Some(shift_units(start_of_unit(today, unit), unit, -1));
    }

    if let Some(rest) = text.strip_prefix("in ") {
        return parse_quantity(rest, today, 1);
    }
    if let Some(rest) = text.strip_suffix(" ago") {
        return parse_quantity(rest, today, -1);
    }
    if let Some(rest) = text.strip_suffix(" from today") {
        return parse_quantity(rest, today, 1);
    }

    None
}

/// A calendar unit that relative expressions can shift by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Day,
    Week,
    Month,
    Year,
}

fn parse_unit(word: &str) -> Option<Unit> {
    match word {
        "day" | "days" => Some(Unit::Day),
        "week" | "weeks" => Some(Unit::Week),
        "month" | "months" => Some(Unit::Month),
        "year" | "years" => Some(Unit::Year),
        _ => None,
    }
}

fn parse_weekday(word: &str) -> Option<time::Weekday> {
    use time::Weekday::{Friday, Monday, Saturday, Sunday, Thursday, Tuesday, Wednesday};
    match word {
        "monday" => Some(Monday),
        "tuesday" => Some(Tuesday),
        "wednesday" => Some(Wednesday),
        "thursday" => Some(Thursday),
        "friday" => Some(Friday),
        "saturday" => Some(Saturday),
        "sunday" => Some(Sunday),
        _ => None,
    }
}

/// Parse `"<N> <unit>"` and shift `today` by `N * sign` units.
fn parse_quantity(text: &str, today: OffsetDateTime, sign: i64) -> Option<OffsetDateTime> {
    let mut parts = text.split(' ');
    let count: i64 = parts.next()?.parse().ok()?;
    let unit = parse_unit(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some(shift_units(today, unit, count * sign))
}

/// Start of the calendar unit containing `day`, in `day`'s offset.
fn start_of_unit(day: OffsetDateTime, unit: Unit) -> OffsetDateTime {
    let date = day.date();
    let start = match unit {
        Unit::Day => date,
        Unit::Week => {
            date - time::Duration::days(i64::from(date.weekday().number_days_from_monday()))
        }
        Unit::Month => date.replace_day(1).unwrap_or(date),
        Unit::Year => date
            .replace_month(time::Month::January)
            .and_then(|d| d.replace_day(1))
            .unwrap_or(date),
    };
    start.midnight().assume_offset(day.offset())
}

/// Shift by calendar units, clamping the day to the target month's length.
///
/// A fixed `Duration` would be wrong here: `31 Jan + 1 month` is not `+28..31
/// days`, it is 28 or 29 Feb depending on the year.
fn shift_units(from: OffsetDateTime, unit: Unit, count: i64) -> OffsetDateTime {
    let date = from.date();
    let shifted = match unit {
        Unit::Day => date + time::Duration::days(count),
        Unit::Week => date + time::Duration::days(count * 7),
        Unit::Month => shift_months(date, count),
        Unit::Year => shift_months(date, count * 12),
    };
    shifted.midnight().assume_offset(from.offset())
}

/// Add `count` calendar months to `date`, clamping the day to the target month.
fn shift_months(date: time::Date, count: i64) -> time::Date {
    let month_index = i64::from(u8::from(date.month()) - 1) + count;
    let year_delta = month_index.div_euclid(12);
    let month = u8::try_from(month_index.rem_euclid(12) + 1).unwrap_or(1);
    let year = i32::try_from(i64::from(date.year()) + year_delta).unwrap_or(date.year());
    let month = time::Month::try_from(month).unwrap_or(date.month());

    // Clamp the day: 31 Jan + 1 month is 28/29 Feb, not an error. The day must
    // be reduced *before* `replace_month`, which itself rejects an out-of-range
    // day for the target month.
    let last_day = month.length(year);
    date.replace_day(date.day().min(last_day))
        .and_then(|d| d.replace_year(year))
        .and_then(|d| d.replace_month(month))
        .unwrap_or(date)
}

/// Parse an ISO 8601 timestamp to a UTC instant.
///
/// Accepts a bare date (`YYYY-MM-DD`, interpreted as midnight UTC) and
/// RFC 3339 date-times. A date-time without an offset is treated as UTC.
pub fn parse_iso8601_utc(raw: &str) -> Result<OffsetDateTime, String> {
    // RFC 3339 requires a `T` between date and time, so a bare date cannot be
    // parsed directly. Normalise it to midnight UTC first.
    let candidate = if raw.len() == 10 && !raw.contains(['T', 't', ' ']) {
        format!("{raw}T00:00:00Z")
    } else {
        raw.to_string()
    };

    OffsetDateTime::parse(&candidate, &Rfc3339)
        .or_else(|_| OffsetDateTime::parse(&format!("{candidate}Z"), &Rfc3339))
        .map_err(|error| error.to_string())
}

/// Parse a `card update` due-date input into an update directive.
///
/// `clear` wins over `raw`; clap rejects passing both. Returns `Unchanged`
/// when neither was given, `Clear` for `--clear-due-date`, and `Set` with an
/// RFC 3339 UTC timestamp otherwise. The value is normalised to UTC so the
/// request is deterministic regardless of the offset supplied.
pub fn parse_due_date_input(
    raw: Option<&str>,
    clear: bool,
    field: &str,
    now: OffsetDateTime,
) -> Result<DueDateUpdate, PlankaError> {
    if clear {
        return Ok(DueDateUpdate::Clear);
    }
    let Some(raw) = raw else {
        return Ok(DueDateUpdate::Unchanged);
    };
    let parsed = parse_bound(raw, field, now)?;
    let formatted = parsed
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .map_err(|error| PlankaError::InvalidOptionValue {
            field: field.to_string(),
            message: error.to_string(),
        })?;
    Ok(DueDateUpdate::Set(formatted))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frozen reference instant: Saturday 2026-04-25, 15:30 at UTC+02:00.
    ///
    /// The offset is deliberately non-UTC so tests catch a parser that derives
    /// the calendar day from the UTC date instead of the local one. The time is
    /// deliberately not midnight so `start_of_day` has something to do.
    fn test_now() -> OffsetDateTime {
        let offset = UtcOffset::from_hms(2, 0, 0).unwrap();
        time::Date::from_calendar_date(2026, time::Month::April, 25)
            .unwrap()
            .with_hms(15, 30, 0)
            .unwrap()
            .assume_offset(offset)
    }

    /// Parse one relative expression against the frozen instant, as a date.
    fn relative(raw: &str) -> time::Date {
        parse_bound(raw, "--due-before", test_now()).unwrap().date()
    }

    fn card(due: Option<&str>) -> Card {
        card_with_state(due, false)
    }

    fn closed_card(due: Option<&str>) -> Card {
        card_with_state(due, true)
    }

    fn card_with_state(due: Option<&str>, is_closed: bool) -> Card {
        Card {
            id: "card-1".to_string(),
            list_id: "list-1".to_string(),
            board_id: "board-1".to_string(),
            name: "Card".to_string(),
            description: None,
            position: 65_536.0,
            due_date: due.map(std::string::ToString::to_string),
            is_due_completed: None,
            is_closed,
            is_subscribed: false,
            creator_user_id: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: None,
        }
    }

    fn nanos(raw: &str) -> i128 {
        parse_iso8601_utc(raw).unwrap().unix_timestamp_nanos()
    }

    #[test]
    fn date_only_is_midnight_utc() {
        assert_eq!(nanos("2026-04-20"), nanos("2026-04-20T00:00:00Z"));
    }

    #[test]
    fn missing_offset_is_utc() {
        assert_eq!(nanos("2026-04-20T12:30:00"), nanos("2026-04-20T12:30:00Z"));
    }

    #[test]
    fn explicit_offset_is_respected() {
        assert_eq!(
            nanos("2026-04-20T02:30:00+02:00"),
            nanos("2026-04-20T00:30:00Z")
        );
    }

    #[test]
    fn fractional_seconds_are_respected() {
        assert_eq!(
            nanos("2026-04-20T00:00:00.500Z"),
            nanos("2026-04-20T00:00:00Z") + 500_000_000
        );
    }

    #[test]
    fn lowercase_rfc3339_markers_are_accepted() {
        assert_eq!(nanos("2026-04-20t00:00:00z"), nanos("2026-04-20T00:00:00Z"));
    }

    #[test]
    fn invalid_timestamp_is_rejected() {
        assert!(parse_iso8601_utc("not-a-date").is_err());
        assert!(parse_iso8601_utc("").is_err());
        assert!(parse_iso8601_utc("2026-13-40").is_err());
    }

    #[test]
    fn unbounded_range_keeps_null_due_dates() {
        let range =
            DueDateRange::parse(None, "--due-after", None, "--due-before", test_now()).unwrap();
        let cards = range.filter(vec![card(None), card(Some("2026-04-20T00:00:00Z"))], false);
        assert_eq!(cards.len(), 2);
    }

    #[test]
    fn bounded_range_drops_null_due_dates() {
        let range = DueDateRange::parse(
            Some("2026-01-01"),
            "--due-after",
            None,
            "--due-before",
            test_now(),
        )
        .unwrap();
        let cards = range.filter(vec![card(None), card(Some("2026-04-20T00:00:00Z"))], false);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, "card-1");
    }

    #[test]
    fn after_bound_is_strict() {
        let range = DueDateRange::parse(
            Some("2026-04-20T00:00:00Z"),
            "--due-after",
            None,
            "--due-before",
            test_now(),
        )
        .unwrap();
        let cards = range.filter(
            vec![
                card(Some("2026-04-19T23:59:59Z")),
                card(Some("2026-04-20T00:00:00Z")),
                card(Some("2026-04-20T00:00:01Z")),
            ],
            false,
        );
        assert_eq!(
            cards
                .iter()
                .map(|c| c.due_date.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["2026-04-20T00:00:01Z"]
        );
    }

    #[test]
    fn before_bound_is_strict() {
        let range = DueDateRange::parse(
            None,
            "--due-after",
            Some("2026-04-20T00:00:00Z"),
            "--due-before",
            test_now(),
        )
        .unwrap();
        let cards = range.filter(
            vec![
                card(Some("2026-04-19T23:59:59Z")),
                card(Some("2026-04-20T00:00:00Z")),
                card(Some("2026-04-20T00:00:01Z")),
            ],
            false,
        );
        assert_eq!(
            cards
                .iter()
                .map(|c| c.due_date.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["2026-04-19T23:59:59Z"]
        );
    }

    #[test]
    fn both_bounds_combine() {
        let range = DueDateRange::parse(
            Some("2026-04-20"),
            "--due-after",
            Some("2026-04-22"),
            "--due-before",
            test_now(),
        )
        .unwrap();
        let cards = range.filter(
            vec![
                card(Some("2026-04-20T00:00:00Z")),
                card(Some("2026-04-21T12:00:00Z")),
                card(Some("2026-04-22T00:00:00Z")),
                card(None),
            ],
            false,
        );
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].due_date.as_deref(), Some("2026-04-21T12:00:00Z"));
    }

    #[test]
    fn unparseable_due_date_is_excluded() {
        let range = DueDateRange::parse(
            Some("2026-01-01"),
            "--due-after",
            None,
            "--due-before",
            test_now(),
        )
        .unwrap();
        let cards = range.filter(
            vec![
                card(Some("2026-04-20T00:00:00Z")),
                card(Some("not-a-timestamp")),
            ],
            false,
        );
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].due_date.as_deref(), Some("2026-04-20T00:00:00Z"));
    }

    /// A due query is a planning question, so a closed card in the window is
    /// dropped unless the caller opts in.
    #[test]
    fn bounded_range_drops_closed_cards_by_default() {
        let range = DueDateRange::parse(
            Some("2026-04-20"),
            "--due-after",
            None,
            "--due-before",
            test_now(),
        )
        .unwrap();
        let cards = range.filter(
            vec![
                card(Some("2026-04-21T00:00:00Z")),
                closed_card(Some("2026-04-21T00:00:00Z")),
            ],
            false,
        );
        assert_eq!(cards.len(), 1);
        assert!(!cards[0].is_closed);
    }

    #[test]
    fn include_closed_keeps_closed_cards_in_a_bounded_range() {
        let range = DueDateRange::parse(
            Some("2026-04-20"),
            "--due-after",
            None,
            "--due-before",
            test_now(),
        )
        .unwrap();
        let cards = range.filter(
            vec![
                card(Some("2026-04-21T00:00:00Z")),
                closed_card(Some("2026-04-21T00:00:00Z")),
            ],
            true,
        );
        assert_eq!(cards.len(), 2);
    }

    /// The exclusion is tied to a bound being set. Without one, the range is a
    /// pass-through and `include_closed` must not change the result.
    #[test]
    fn unbounded_range_keeps_closed_cards_regardless_of_include_closed() {
        let range =
            DueDateRange::parse(None, "--due-after", None, "--due-before", test_now()).unwrap();
        let cards = range.filter(vec![closed_card(Some("2026-04-21T00:00:00Z"))], false);
        assert_eq!(cards.len(), 1);
        let cards = range.filter(vec![closed_card(Some("2026-04-21T00:00:00Z"))], true);
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn parse_due_date_input_uses_utc() {
        assert_eq!(
            parse_due_date_input(Some("2026-04-20"), false, "--due-date", test_now()).unwrap(),
            DueDateUpdate::Set("2026-04-20T00:00:00Z".to_string())
        );
        assert_eq!(
            parse_due_date_input(
                Some("2026-04-20T02:30:00+02:00"),
                false,
                "--due-date",
                test_now()
            )
            .unwrap(),
            DueDateUpdate::Set("2026-04-20T00:30:00Z".to_string())
        );
    }

    #[test]
    fn parse_due_date_input_clear_flag_clears() {
        assert_eq!(
            parse_due_date_input(None, true, "--due-date", test_now()).unwrap(),
            DueDateUpdate::Clear
        );
    }

    /// `--clear-due-date` takes precedence over a stray value; clap rejects
    /// passing both, so this only guards the helper's own contract.
    #[test]
    fn parse_due_date_input_clear_wins_over_value() {
        assert_eq!(
            parse_due_date_input(Some("2026-04-20"), true, "--due-date", test_now()).unwrap(),
            DueDateUpdate::Clear
        );
    }

    #[test]
    fn parse_due_date_input_absent_is_unchanged() {
        assert_eq!(
            parse_due_date_input(None, false, "--due-date", test_now()).unwrap(),
            DueDateUpdate::Unchanged
        );
    }

    /// A bare `-` is not a clear sentinel any more — it is an invalid date.
    #[test]
    fn parse_due_date_input_dash_is_not_a_sentinel() {
        let error = parse_due_date_input(Some("-"), false, "--due-date", test_now()).unwrap_err();
        assert_eq!(error.error_type(), "InvalidOptionValue");
    }

    #[test]
    fn parse_due_date_input_rejects_invalid() {
        let error =
            parse_due_date_input(Some("nope"), false, "--due-date", test_now()).unwrap_err();
        assert_eq!(error.error_type(), "InvalidOptionValue");
        assert!(error.to_string().contains("--due-date"));
    }

    #[test]
    fn parse_error_names_the_flag() {
        let error = DueDateRange::parse(
            Some("nope"),
            "--due-after",
            None,
            "--due-before",
            test_now(),
        )
        .unwrap_err();
        assert_eq!(error.error_type(), "InvalidOptionValue");
        assert!(error.to_string().contains("--due-after"));
        assert_eq!(error.exit_code(), 2);
    }

    // ── Relative dates ──────────────────────────────────────────────
    //
    // Reference: Saturday 2026-04-25, 15:30 UTC+02:00.

    #[test]
    fn relative_day_words() {
        assert_eq!(
            relative("today"),
            time::Date::from_calendar_date(2026, time::Month::April, 25).unwrap()
        );
        assert_eq!(
            relative("tomorrow"),
            time::Date::from_calendar_date(2026, time::Month::April, 26).unwrap()
        );
        assert_eq!(
            relative("yesterday"),
            time::Date::from_calendar_date(2026, time::Month::April, 24).unwrap()
        );
    }

    /// `now` is the exception to "start of day": it returns the reference
    /// instant itself, time of day and offset included.
    #[test]
    fn relative_now_keeps_the_time_of_day() {
        let parsed = parse_bound("now", "--due-before", test_now()).unwrap();
        assert_eq!(parsed, test_now());
        assert_eq!(parsed.time(), time::Time::from_hms(15, 30, 0).unwrap());
        assert_eq!(parsed.offset(), test_now().offset());
        // Contrast with `today`, which collapses to midnight.
        assert_ne!(parsed, start_of_day(test_now()));
    }

    /// Case and surrounding whitespace must not matter.
    #[test]
    fn relative_day_words_are_case_and_space_insensitive() {
        assert_eq!(relative("  TODAY  "), relative("today"));
        assert_eq!(relative("ToMoRrOw"), relative("tomorrow"));
        assert_eq!(
            parse_bound("  NOW  ", "--due-before", test_now()).unwrap(),
            parse_bound("now", "--due-before", test_now()).unwrap()
        );
    }

    #[test]
    fn relative_days_from_today() {
        assert_eq!(
            relative("3 days from today"),
            time::Date::from_calendar_date(2026, time::Month::April, 28).unwrap()
        );
        assert_eq!(
            relative("in 3 days"),
            time::Date::from_calendar_date(2026, time::Month::April, 28).unwrap()
        );
        assert_eq!(
            relative("3 days ago"),
            time::Date::from_calendar_date(2026, time::Month::April, 22).unwrap()
        );
    }

    #[test]
    fn relative_weeks() {
        assert_eq!(
            relative("2 weeks from today"),
            time::Date::from_calendar_date(2026, time::Month::May, 9).unwrap()
        );
        assert_eq!(
            relative("in 2 weeks"),
            time::Date::from_calendar_date(2026, time::Month::May, 9).unwrap()
        );
        assert_eq!(
            relative("2 weeks ago"),
            time::Date::from_calendar_date(2026, time::Month::April, 11).unwrap()
        );
        assert_eq!(
            relative("next week"),
            time::Date::from_calendar_date(2026, time::Month::April, 27).unwrap()
        );
        assert_eq!(
            relative("last week"),
            time::Date::from_calendar_date(2026, time::Month::April, 13).unwrap()
        );
        assert_eq!(
            relative("this week"),
            time::Date::from_calendar_date(2026, time::Month::April, 20).unwrap()
        );
    }

    #[test]
    fn relative_months() {
        assert_eq!(
            relative("next month"),
            time::Date::from_calendar_date(2026, time::Month::May, 1).unwrap()
        );
        assert_eq!(
            relative("last month"),
            time::Date::from_calendar_date(2026, time::Month::March, 1).unwrap()
        );
        assert_eq!(
            relative("this month"),
            time::Date::from_calendar_date(2026, time::Month::April, 1).unwrap()
        );
        assert_eq!(
            relative("in 2 months"),
            time::Date::from_calendar_date(2026, time::Month::June, 25).unwrap()
        );
    }

    #[test]
    fn relative_years() {
        assert_eq!(
            relative("next year"),
            time::Date::from_calendar_date(2027, time::Month::January, 1).unwrap()
        );
        assert_eq!(
            relative("this year"),
            time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap()
        );
        assert_eq!(
            relative("in 1 year"),
            time::Date::from_calendar_date(2027, time::Month::April, 25).unwrap()
        );
    }

    /// Bare weekday names mean the *next* occurrence, never today.
    #[test]
    fn relative_weekdays_are_next_occurrence() {
        // 2026-04-25 is a Saturday.
        assert_eq!(
            relative("sunday"),
            time::Date::from_calendar_date(2026, time::Month::April, 26).unwrap()
        );
        assert_eq!(
            relative("monday"),
            time::Date::from_calendar_date(2026, time::Month::April, 27).unwrap()
        );
        assert_eq!(
            relative("friday"),
            time::Date::from_calendar_date(2026, time::Month::May, 1).unwrap()
        );
        // Same weekday as today: a full week out, not zero days.
        assert_eq!(
            relative("saturday"),
            time::Date::from_calendar_date(2026, time::Month::May, 2).unwrap()
        );
    }

    /// A relative date resolves to the start of the local day, not the UTC day.
    #[test]
    fn relative_dates_use_the_reference_offset() {
        let parsed = parse_bound("today", "--due-before", test_now()).unwrap();
        assert_eq!(parsed.offset(), test_now().offset());
        assert_eq!(parsed.time(), time::Time::MIDNIGHT);
        // 2026-04-25T00:00+02:00 is 2026-04-24T22:00Z.
        assert_eq!(nanos("2026-04-24T22:00:00Z"), parsed.unix_timestamp_nanos());
    }

    /// Calendar months clamp to the target month's length rather than erroring
    /// or silently rolling over into the following month.
    #[test]
    fn month_arithmetic_clamps_to_month_end() {
        let jan31 = time::Date::from_calendar_date(2026, time::Month::January, 31)
            .unwrap()
            .midnight()
            .assume_utc();
        assert_eq!(
            shift_units(jan31, Unit::Month, 1).date(),
            time::Date::from_calendar_date(2026, time::Month::February, 28).unwrap()
        );

        // Leap year: 2028 is divisible by 4.
        let jan31_leap = time::Date::from_calendar_date(2028, time::Month::January, 31)
            .unwrap()
            .midnight()
            .assume_utc();
        assert_eq!(
            shift_units(jan31_leap, Unit::Month, 1).date(),
            time::Date::from_calendar_date(2028, time::Month::February, 29).unwrap()
        );
    }

    #[test]
    fn month_arithmetic_rolls_over_the_year() {
        let dec15 = time::Date::from_calendar_date(2026, time::Month::December, 15)
            .unwrap()
            .midnight()
            .assume_utc();
        assert_eq!(
            shift_units(dec15, Unit::Month, 1).date(),
            time::Date::from_calendar_date(2027, time::Month::January, 15).unwrap()
        );
        assert_eq!(
            shift_units(dec15, Unit::Month, 13).date(),
            time::Date::from_calendar_date(2028, time::Month::January, 15).unwrap()
        );
    }

    #[test]
    fn unrecognised_relative_words_are_rejected() {
        for raw in [
            "next friday",
            "end of month",
            "+3d",
            "sometime soon",
            "in 3",
            "in days",
            "3 from today",
            "2 weeks from yesterday",
        ] {
            let error = parse_bound(raw, "--due-before", test_now()).unwrap_err();
            assert_eq!(
                error.error_type(),
                "InvalidOptionValue",
                "{raw} should be rejected"
            );
            assert!(
                error.to_string().contains("relative date"),
                "{raw} error should mention the relative grammar"
            );
        }
    }

    /// Adding relative parsing must not disturb any existing ISO 8601 form.
    #[test]
    fn iso8601_still_parses_identically() {
        for raw in [
            "2026-04-20",
            "2026-04-20T12:30:00Z",
            "2026-04-20T12:30:00+02:00",
            "2026-04-20T12:30:00.500Z",
        ] {
            assert_eq!(
                parse_bound(raw, "--due-before", test_now()).unwrap(),
                parse_iso8601_utc(raw).unwrap(),
                "{raw} must parse the same as before"
            );
        }
    }

    /// Relative expressions also work for `card update --due-date`.
    #[test]
    fn due_date_input_accepts_relative_dates() {
        assert_eq!(
            parse_due_date_input(Some("tomorrow"), false, "--due-date", test_now()).unwrap(),
            DueDateUpdate::Set("2026-04-25T22:00:00Z".to_string())
        );
    }

    /// `now` on the write path keeps the time of day, normalised to UTC.
    #[test]
    fn due_date_input_accepts_now() {
        // test_now() is 2026-04-25T15:30+02:00, i.e. 13:30 UTC.
        assert_eq!(
            parse_due_date_input(Some("now"), false, "--due-date", test_now()).unwrap(),
            DueDateUpdate::Set("2026-04-25T13:30:00Z".to_string())
        );
    }

    /// Shifting backwards from a month end must land on the last valid day of
    /// the target month, not silently collapse to an earlier day.
    #[test]
    fn month_arithmetic_shifts_backwards_from_month_end() {
        let mar31 = time::Date::from_calendar_date(2026, time::Month::March, 31)
            .unwrap()
            .midnight()
            .assume_utc();
        assert_eq!(
            shift_units(mar31, Unit::Month, -1).date(),
            time::Date::from_calendar_date(2026, time::Month::February, 28).unwrap()
        );

        // 29 Feb in a leap year back one month is 29 Jan, not 28 Jan.
        let feb29 = time::Date::from_calendar_date(2028, time::Month::February, 29)
            .unwrap()
            .midnight()
            .assume_utc();
        assert_eq!(
            shift_units(feb29, Unit::Month, -1).date(),
            time::Date::from_calendar_date(2028, time::Month::January, 29).unwrap()
        );
    }
}
