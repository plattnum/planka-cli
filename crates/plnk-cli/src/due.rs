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
    /// parse failure maps to the right `InvalidOptionValue` field. `clock`
    /// supplies the reference instant and local offsets; callers use
    /// [`LocalClock::system`], tests pass a frozen one.
    pub fn parse(
        after: Option<&str>,
        after_field: &str,
        before: Option<&str>,
        before_field: &str,
        clock: LocalClock,
    ) -> Result<Self, PlankaError> {
        Ok(Self {
            after: after
                .map(|raw| parse_bound(raw, after_field, clock))
                .transpose()?,
            before: before
                .map(|raw| parse_bound(raw, before_field, clock))
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
/// Relative forms are tried first, then ISO 8601. `clock` supplies the
/// reference instant for relative expressions and the local offset of each
/// resolved date.
fn parse_bound(raw: &str, field: &str, clock: LocalClock) -> Result<OffsetDateTime, PlankaError> {
    match parse_relative(raw, clock) {
        Ok(Some(relative)) => return Ok(relative),
        Ok(None) => {}
        Err(reason) => return Err(invalid_due_date(raw, field, &reason)),
    }

    // A bare date means midnight in the caller's timezone, matching relative
    // forms such as `today`. `parse_iso8601_utc` reads it as midnight UTC, which
    // is only correct for timestamps already stored on the server.
    if is_bare_date(raw) {
        let midnight =
            parse_iso8601_utc(raw).map_err(|reason| invalid_due_date(raw, field, &reason))?;
        return Ok(clock.local_midnight(midnight.date()));
    }

    parse_iso8601_utc(raw).map_err(|reason| invalid_due_date(raw, field, &reason))
}

/// Build the `InvalidOptionValue` error for a rejected due-date input.
fn invalid_due_date(raw: &str, field: &str, reason: &str) -> PlankaError {
    PlankaError::InvalidOptionValue {
        field: field.to_string(),
        message: format!("'{raw}' is not a valid due date: {DUE_DATE_GRAMMAR} ({reason})"),
    }
}

/// Whether `raw` is a bare `YYYY-MM-DD` date with no time component.
fn is_bare_date(raw: &str) -> bool {
    raw.len() == 10 && !raw.contains(['T', 't', ' '])
}

/// The reference instant for relative due dates, plus the local UTC offset at
/// any other instant.
///
/// The offset is looked up per date, not taken from `now`: across a DST change
/// the offset on the target date differs from today's, and reusing today's would
/// move local midnight by an hour, onto the previous day.
#[derive(Debug, Clone, Copy)]
pub struct LocalClock {
    now: OffsetDateTime,
    /// Local offset in force at an instant, or `None` when it cannot be
    /// determined.
    offset_at: fn(OffsetDateTime) -> Option<UtcOffset>,
}

impl LocalClock {
    /// The system clock and timezone.
    ///
    /// `time` reads the offset from `TZ` / `/etc/localtime`. When the current
    /// offset cannot be determined, everything falls back to UTC.
    ///
    /// `time` documents the offset lookup as unsound if another thread mutates
    /// `TZ` concurrently; `plnk` never does.
    #[must_use]
    pub fn system() -> Self {
        match OffsetDateTime::now_local() {
            Ok(now) => Self {
                now,
                offset_at: |instant| UtcOffset::local_offset_at(instant).ok(),
            },
            Err(_) => Self {
                now: OffsetDateTime::now_utc(),
                offset_at: |_| Some(UtcOffset::UTC),
            },
        }
    }

    /// Midnight at the start of `date`, in the local offset in force then.
    ///
    /// The offset depends on the instant, which depends on the offset, so start
    /// from today's offset and refine. One refinement settles every ordinary DST
    /// change; the second covers a transition that lands exactly on midnight.
    fn local_midnight(self, date: time::Date) -> OffsetDateTime {
        let mut offset = self.now.offset();
        for _ in 0..2 {
            let resolved =
                (self.offset_at)(date.midnight().assume_offset(offset)).unwrap_or(offset);
            if resolved == offset {
                break;
            }
            offset = resolved;
        }
        date.midnight().assume_offset(offset)
    }
}

/// Parse a relative date expression against `now`.
///
/// Returns `Ok(None)` for anything the grammar does not recognise, leaving ISO
/// 8601 parsing to the caller, and `Err` when a recognised expression resolves
/// to a date outside the representable range. Bare relative dates resolve to the
/// start of the day in the local offset in force on that day; `now` is the
/// exception and returns the reference instant itself, keeping the time of day.
fn parse_relative(raw: &str, clock: LocalClock) -> Result<Option<OffsetDateTime>, String> {
    let text = raw.trim().to_ascii_lowercase();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let now = clock.now;
    let today = now.date();
    let at_midnight = |date| Ok(Some(clock.local_midnight(date)));

    match text.as_str() {
        "now" => return Ok(Some(now)),
        "today" => return at_midnight(today),
        "tomorrow" => return at_midnight(shift(today, Unit::Day, 1)?),
        "yesterday" => return at_midnight(shift(today, Unit::Day, -1)?),
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
        return at_midnight(shift(today, Unit::Day, delta)?);
    }

    if let Some(unit) = text.strip_prefix("this ").and_then(parse_unit) {
        return at_midnight(start_of_unit(today, unit));
    }
    if let Some(unit) = text.strip_prefix("next ").and_then(parse_unit) {
        return at_midnight(shift(start_of_unit(today, unit), unit, 1)?);
    }
    if let Some(unit) = text.strip_prefix("last ").and_then(parse_unit) {
        return at_midnight(shift(start_of_unit(today, unit), unit, -1)?);
    }

    let quantity = if let Some(rest) = text.strip_prefix("in ") {
        parse_quantity(rest, today, 1)?
    } else if let Some(rest) = text.strip_suffix(" ago") {
        parse_quantity(rest, today, -1)?
    } else if let Some(rest) = text.strip_suffix(" from today") {
        parse_quantity(rest, today, 1)?
    } else {
        None
    };
    Ok(quantity.map(|date| clock.local_midnight(date)))
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
///
/// Returns `Ok(None)` when `text` is not a quantity expression, and `Err` when it
/// is one but the resulting date cannot be represented.
fn parse_quantity(text: &str, today: time::Date, sign: i64) -> Result<Option<time::Date>, String> {
    let mut parts = text.split(' ');
    let Some(Ok(count)) = parts.next().map(str::parse::<i64>) else {
        return Ok(None);
    };
    let Some(unit) = parts.next().and_then(parse_unit) else {
        return Ok(None);
    };
    if parts.next().is_some() {
        return Ok(None);
    }
    let count = count.checked_mul(sign).ok_or_else(out_of_range)?;
    shift(today, unit, count).map(Some)
}

/// First day of the calendar unit containing `date`.
fn start_of_unit(date: time::Date, unit: Unit) -> time::Date {
    match unit {
        Unit::Day => date,
        Unit::Week => {
            date - time::Duration::days(i64::from(date.weekday().number_days_from_monday()))
        }
        Unit::Month => date.replace_day(1).unwrap_or(date),
        Unit::Year => date
            .replace_month(time::Month::January)
            .and_then(|d| d.replace_day(1))
            .unwrap_or(date),
    }
}

/// The reason a recognised relative form cannot be represented.
const RELATIVE_OUT_OF_RANGE: &str =
    "the resulting date is outside the supported range (years -9999 to 9999)";

fn out_of_range() -> String {
    RELATIVE_OUT_OF_RANGE.to_string()
}

/// Build a `Duration` of `days` days, or `None` when it does not fit.
///
/// `Duration::days` panics on overflow, so multiply before constructing.
fn duration_days(days: i64) -> Option<time::Duration> {
    days.checked_mul(86_400).map(time::Duration::seconds)
}

/// Shift `from` by `count` units, reporting an out-of-range result as an error.
fn shift(from: time::Date, unit: Unit, count: i64) -> Result<time::Date, String> {
    shift_units(from, unit, count).ok_or_else(out_of_range)
}

/// Shift by calendar units, clamping the day to the target month's length.
///
/// Returns `None` when the result leaves the range `time::Date` can represent.
/// A fixed `Duration` would be wrong for months: `31 Jan + 1 month` is not
/// `+28..31 days`, it is 28 or 29 Feb depending on the year.
fn shift_units(date: time::Date, unit: Unit, count: i64) -> Option<time::Date> {
    match unit {
        Unit::Day => date.checked_add(duration_days(count)?),
        Unit::Week => date.checked_add(duration_days(count.checked_mul(7)?)?),
        Unit::Month => shift_months(date, count),
        Unit::Year => shift_months(date, count.checked_mul(12)?),
    }
}

/// Add `count` calendar months to `date`, clamping the day to the target month.
///
/// Returns `None` when the result is outside the representable range.
fn shift_months(date: time::Date, count: i64) -> Option<time::Date> {
    let month_index = i64::from(u8::from(date.month()) - 1).checked_add(count)?;
    let year_delta = month_index.div_euclid(12);
    let month = u8::try_from(month_index.rem_euclid(12) + 1).ok()?;
    let year = i32::try_from(i64::from(date.year()) + year_delta).ok()?;
    let month = time::Month::try_from(month).ok()?;

    // Clamp the day: 31 Jan + 1 month is 28/29 Feb, not an error. The day must
    // be reduced *before* `replace_month`, which itself rejects an out-of-range
    // day for the target month. `replace_year` rejects a year outside
    // -9999..=9999, which is how an out-of-range shift becomes `None`.
    let last_day = month.length(year);
    date.replace_day(date.day().min(last_day))
        .ok()?
        .replace_year(year)
        .ok()?
        .replace_month(month)
        .ok()
}

/// Parse an ISO 8601 timestamp to a UTC instant.
///
/// Accepts a bare date (`YYYY-MM-DD`, interpreted as midnight UTC), RFC 3339
/// date-times, and the seconds-less `YYYY-MM-DDThh:mm`. A date-time without an
/// offset is treated as UTC. The UTC reading of a bare date is right for values
/// already stored on the server; user input goes through [`parse_bound`], which
/// applies the caller's local offset instead.
pub fn parse_iso8601_utc(raw: &str) -> Result<OffsetDateTime, String> {
    // RFC 3339 requires a `T` between date and time and seconds in the time, so a
    // bare date and a seconds-less time cannot be parsed directly.
    let candidate = if is_bare_date(raw) {
        format!("{raw}T00:00:00Z")
    } else {
        insert_missing_seconds(raw)
    };

    OffsetDateTime::parse(&candidate, &Rfc3339)
        .or_else(|_| OffsetDateTime::parse(&format!("{candidate}Z"), &Rfc3339))
        .map_err(|error| error.to_string())
}

/// Insert `:00` when a date-time carries `hh:mm` but no seconds.
///
/// RFC 3339 requires seconds, but the documented grammar accepts
/// `YYYY-MM-DDThh:mm`. Anything else is returned unchanged so the parser can
/// report the original value.
fn insert_missing_seconds(raw: &str) -> String {
    let Some(t_pos) = raw.find(['T', 't']) else {
        return raw.to_string();
    };
    let time = &raw[t_pos + 1..];
    // An offset starts at the first `Z`, `+`, or `-`; a bare time has none.
    let offset_start = time.find(['Z', 'z', '+', '-']).unwrap_or(time.len());
    let clock = &time[..offset_start];
    if clock.len() == 5 && clock.as_bytes().get(2) == Some(&b':') {
        format!(
            "{}:00{}",
            &raw[..t_pos + 1 + offset_start],
            &time[offset_start..]
        )
    } else {
        raw.to_string()
    }
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
    clock: LocalClock,
) -> Result<DueDateUpdate, PlankaError> {
    if clear {
        return Ok(DueDateUpdate::Clear);
    }
    let Some(raw) = raw else {
        return Ok(DueDateUpdate::Unchanged);
    };
    let parsed = parse_bound(raw, field, clock)?;
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

    /// A clock frozen at `now` whose offset never changes, so results depend only
    /// on `now`.
    fn fixed(now: OffsetDateTime) -> LocalClock {
        LocalClock {
            now,
            offset_at: |_| None,
        }
    }

    /// A clock frozen at `now` with a fake DST rule: UTC+02:00 from April to
    /// October (by UTC month), UTC+01:00 otherwise. Close enough to Central
    /// European time to exercise a change of offset between today and the
    /// target date.
    fn dst(now: OffsetDateTime) -> LocalClock {
        LocalClock {
            now,
            offset_at: |instant| {
                let summer =
                    (4..=10).contains(&u8::from(instant.to_offset(UtcOffset::UTC).month()));
                Some(UtcOffset::from_hms(if summer { 2 } else { 1 }, 0, 0).unwrap())
            },
        }
    }

    /// A bare date across a DST change gets the offset in force on that date,
    /// not today's. `test_now()` is in summer (UTC+02:00); 1 December is in
    /// winter (UTC+01:00), so its local midnight is 23:00Z the day before, not
    /// 22:00Z.
    #[test]
    fn bare_date_uses_the_offset_on_that_date() {
        assert_eq!(
            parse_due_date_input(Some("2026-12-01"), false, "--due-date", dst(test_now())).unwrap(),
            DueDateUpdate::Set("2026-11-30T23:00:00Z".to_string())
        );
        // Same season as today: unchanged.
        assert_eq!(
            parse_due_date_input(Some("2026-05-01"), false, "--due-date", dst(test_now())).unwrap(),
            DueDateUpdate::Set("2026-04-30T22:00:00Z".to_string())
        );
    }

    /// Relative forms that land across a DST change get that date's offset too.
    #[test]
    fn relative_date_uses_the_offset_on_that_date() {
        let parsed = parse_bound("in 8 months", "--due-before", dst(test_now())).unwrap();
        assert_eq!(
            parsed.date(),
            time::Date::from_calendar_date(2026, time::Month::December, 25).unwrap()
        );
        assert_eq!(parsed.offset(), UtcOffset::from_hms(1, 0, 0).unwrap());
        assert_eq!(parsed.time(), time::Time::MIDNIGHT);
    }

    /// Parse one relative expression against the frozen instant, as a date.
    fn relative(raw: &str) -> time::Date {
        parse_bound(raw, "--due-before", fixed(test_now()))
            .unwrap()
            .date()
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

    /// The low-level parser still reads a bare date as midnight UTC; it is used
    /// for values already stored on the server. User-facing input goes through
    /// `parse_bound`, which applies the local offset.
    #[test]
    fn parse_iso8601_utc_reads_bare_date_as_utc() {
        assert_eq!(nanos("2026-04-20"), nanos("2026-04-20T00:00:00Z"));
    }

    /// A user-supplied bare date means midnight in the caller's timezone, so it
    /// agrees with `today`. `test_now()` is UTC+02:00.
    #[test]
    fn bare_date_is_local_midnight() {
        let parsed = parse_bound("2026-04-20", "--due-before", fixed(test_now())).unwrap();
        assert_eq!(parsed.offset(), test_now().offset());
        assert_eq!(parsed.time(), time::Time::MIDNIGHT);
        // 2026-04-20T00:00+02:00 is 2026-04-19T22:00Z.
        assert_eq!(nanos("2026-04-19T22:00:00Z"), parsed.unix_timestamp_nanos());
    }

    #[test]
    fn missing_offset_is_utc() {
        assert_eq!(nanos("2026-04-20T12:30:00"), nanos("2026-04-20T12:30:00Z"));
    }

    /// The documented grammar accepts `hh:mm` without seconds, even though RFC
    /// 3339 requires them. Offsets and the bare form must both work.
    #[test]
    fn seconds_less_time_is_accepted() {
        assert_eq!(nanos("2026-04-30T12:00"), nanos("2026-04-30T12:00:00Z"));
        assert_eq!(nanos("2026-04-30T12:00Z"), nanos("2026-04-30T12:00:00Z"));
        assert_eq!(
            nanos("2026-04-30T12:00+02:00"),
            nanos("2026-04-30T10:00:00Z")
        );
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
            DueDateRange::parse(None, "--due-after", None, "--due-before", fixed(test_now()))
                .unwrap();
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
            fixed(test_now()),
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
            fixed(test_now()),
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
            fixed(test_now()),
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
            Some("2026-04-20T00:00:00Z"),
            "--due-after",
            Some("2026-04-22T00:00:00Z"),
            "--due-before",
            fixed(test_now()),
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
            fixed(test_now()),
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
            fixed(test_now()),
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
            fixed(test_now()),
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
            DueDateRange::parse(None, "--due-after", None, "--due-before", fixed(test_now()))
                .unwrap();
        let cards = range.filter(vec![closed_card(Some("2026-04-21T00:00:00Z"))], false);
        assert_eq!(cards.len(), 1);
        let cards = range.filter(vec![closed_card(Some("2026-04-21T00:00:00Z"))], true);
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn parse_due_date_input_uses_utc() {
        assert_eq!(
            parse_due_date_input(
                Some("2026-04-20T02:30:00+02:00"),
                false,
                "--due-date",
                fixed(test_now())
            )
            .unwrap(),
            DueDateUpdate::Set("2026-04-20T00:30:00Z".to_string())
        );
    }

    /// A bare date is local midnight before it is normalised to UTC.
    #[test]
    fn parse_due_date_input_bare_date_is_local_midnight() {
        // test_now() is UTC+02:00, so 2026-04-20T00:00+02:00 is 22:00Z the day before.
        assert_eq!(
            parse_due_date_input(Some("2026-04-20"), false, "--due-date", fixed(test_now()))
                .unwrap(),
            DueDateUpdate::Set("2026-04-19T22:00:00Z".to_string())
        );
    }

    #[test]
    fn parse_due_date_input_clear_flag_clears() {
        assert_eq!(
            parse_due_date_input(None, true, "--due-date", fixed(test_now())).unwrap(),
            DueDateUpdate::Clear
        );
    }

    /// `--clear-due-date` takes precedence over a stray value; clap rejects
    /// passing both, so this only guards the helper's own contract.
    #[test]
    fn parse_due_date_input_clear_wins_over_value() {
        assert_eq!(
            parse_due_date_input(Some("2026-04-20"), true, "--due-date", fixed(test_now()))
                .unwrap(),
            DueDateUpdate::Clear
        );
    }

    #[test]
    fn parse_due_date_input_absent_is_unchanged() {
        assert_eq!(
            parse_due_date_input(None, false, "--due-date", fixed(test_now())).unwrap(),
            DueDateUpdate::Unchanged
        );
    }

    /// A bare `-` is not a clear sentinel any more — it is an invalid date.
    #[test]
    fn parse_due_date_input_dash_is_not_a_sentinel() {
        let error =
            parse_due_date_input(Some("-"), false, "--due-date", fixed(test_now())).unwrap_err();
        assert_eq!(error.error_type(), "InvalidOptionValue");
    }

    #[test]
    fn parse_due_date_input_rejects_invalid() {
        let error =
            parse_due_date_input(Some("nope"), false, "--due-date", fixed(test_now())).unwrap_err();
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
            fixed(test_now()),
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
        let parsed = parse_bound("now", "--due-before", fixed(test_now())).unwrap();
        assert_eq!(parsed, test_now());
        assert_eq!(parsed.time(), time::Time::from_hms(15, 30, 0).unwrap());
        assert_eq!(parsed.offset(), test_now().offset());
        // Contrast with `today`, which collapses to midnight.
        assert_ne!(parsed, test_now().replace_time(time::Time::MIDNIGHT));
    }

    /// Case and surrounding whitespace must not matter.
    #[test]
    fn relative_day_words_are_case_and_space_insensitive() {
        assert_eq!(relative("  TODAY  "), relative("today"));
        assert_eq!(relative("ToMoRrOw"), relative("tomorrow"));
        assert_eq!(
            parse_bound("  NOW  ", "--due-before", fixed(test_now())).unwrap(),
            parse_bound("now", "--due-before", fixed(test_now())).unwrap()
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
        let parsed = parse_bound("today", "--due-before", fixed(test_now())).unwrap();
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
            shift_units(jan31.date(), Unit::Month, 1).unwrap(),
            time::Date::from_calendar_date(2026, time::Month::February, 28).unwrap()
        );

        // Leap year: 2028 is divisible by 4.
        let jan31_leap = time::Date::from_calendar_date(2028, time::Month::January, 31)
            .unwrap()
            .midnight()
            .assume_utc();
        assert_eq!(
            shift_units(jan31_leap.date(), Unit::Month, 1).unwrap(),
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
            shift_units(dec15.date(), Unit::Month, 1).unwrap(),
            time::Date::from_calendar_date(2027, time::Month::January, 15).unwrap()
        );
        assert_eq!(
            shift_units(dec15.date(), Unit::Month, 13).unwrap(),
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
            let error = parse_bound(raw, "--due-before", fixed(test_now())).unwrap_err();
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

    /// A recognised relative form whose result leaves the representable range is
    /// a validation error, not a panic and not a silent fallback to today.
    #[test]
    fn out_of_range_relative_dates_are_rejected() {
        for raw in [
            "in 10000000 days",
            "in 9999999999999999 weeks",
            "in 100000 years",
            "100000 years ago",
        ] {
            let error = parse_bound(raw, "--due-before", fixed(test_now())).unwrap_err();
            assert_eq!(
                error.error_type(),
                "InvalidOptionValue",
                "{raw} should be rejected"
            );
            assert_eq!(error.exit_code(), 2, "{raw} should exit 2");
            assert!(
                error.to_string().contains("outside the supported range"),
                "{raw} error should explain the range: {error}"
            );
        }
    }

    /// `next month` means the start of the next month, so it does not clamp the
    /// day of the current month; only `in N months` does.
    #[test]
    fn next_month_is_the_start_of_the_next_month() {
        let jan31 = time::Date::from_calendar_date(2026, time::Month::January, 31)
            .unwrap()
            .with_hms(9, 0, 0)
            .unwrap()
            .assume_utc();
        assert_eq!(
            parse_bound("next month", "--due-before", fixed(jan31))
                .unwrap()
                .date(),
            time::Date::from_calendar_date(2026, time::Month::February, 1).unwrap()
        );
        assert_eq!(
            parse_bound("in 1 month", "--due-before", fixed(jan31))
                .unwrap()
                .date(),
            time::Date::from_calendar_date(2026, time::Month::February, 28).unwrap()
        );
    }

    /// Adding relative parsing must not disturb the ISO 8601 forms. A bare date
    /// is deliberately excluded: it now resolves to local midnight, not UTC.
    #[test]
    fn iso8601_still_parses_identically() {
        for raw in [
            "2026-04-20T12:30:00Z",
            "2026-04-20T12:30:00+02:00",
            "2026-04-20T12:30:00.500Z",
        ] {
            assert_eq!(
                parse_bound(raw, "--due-before", fixed(test_now())).unwrap(),
                parse_iso8601_utc(raw).unwrap(),
                "{raw} must parse the same as before"
            );
        }
    }

    /// Relative expressions also work for `card update --due-date`.
    #[test]
    fn due_date_input_accepts_relative_dates() {
        assert_eq!(
            parse_due_date_input(Some("tomorrow"), false, "--due-date", fixed(test_now())).unwrap(),
            DueDateUpdate::Set("2026-04-25T22:00:00Z".to_string())
        );
    }

    /// `now` on the write path keeps the time of day, normalised to UTC.
    #[test]
    fn due_date_input_accepts_now() {
        // test_now() is 2026-04-25T15:30+02:00, i.e. 13:30 UTC.
        assert_eq!(
            parse_due_date_input(Some("now"), false, "--due-date", fixed(test_now())).unwrap(),
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
            shift_units(mar31.date(), Unit::Month, -1).unwrap(),
            time::Date::from_calendar_date(2026, time::Month::February, 28).unwrap()
        );

        // 29 Feb in a leap year back one month is 29 Jan, not 28 Jan.
        let feb29 = time::Date::from_calendar_date(2028, time::Month::February, 29)
            .unwrap()
            .midnight()
            .assume_utc();
        assert_eq!(
            shift_units(feb29.date(), Unit::Month, -1).unwrap(),
            time::Date::from_calendar_date(2028, time::Month::January, 29).unwrap()
        );
    }
}
