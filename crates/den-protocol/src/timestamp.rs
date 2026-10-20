//! Wire timestamps (Spec 02 §3): RFC 3339 UTC with milliseconds and `Z`, e.g. `2026-10-18T20:15:03.120Z`.

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};
use ts_rs::TS;

/// An instant in UTC with millisecond precision. Serialized as RFC 3339 with exactly three fractional
/// digits and a `Z` suffix; any RFC 3339 offset is accepted on input and normalised to UTC.
/// Sub-millisecond precision is truncated on construction, so a value always equals its wire form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "string")]
pub struct Timestamp(OffsetDateTime);

/// The JSON Schema `pattern` every emitted timestamp matches.
pub const TIMESTAMP_PATTERN: &str = "^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}\\.\\d{3}Z$";

const WIRE_FORMAT: &[time::format_description::BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

impl Timestamp {
    /// Wraps an instant, converting it to UTC and truncating it to whole milliseconds (toward negative
    /// infinity, so the wire form and [`Timestamp::unix_millis`] agree before 1970 too).
    pub fn new(instant: OffsetDateTime) -> Self {
        let nanos = instant.unix_timestamp_nanos();
        let truncated = nanos.div_euclid(1_000_000) * 1_000_000;
        let instant = OffsetDateTime::from_unix_timestamp_nanos(truncated).unwrap_or(instant);
        Self(instant.to_offset(UtcOffset::UTC))
    }

    /// The current time.
    pub fn now() -> Self {
        Self::new(OffsetDateTime::now_utc())
    }

    /// Builds a timestamp from milliseconds since the Unix epoch.
    pub fn from_unix_millis(millis: i64) -> Option<Self> {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
            .ok()
            .map(Self::new)
    }

    /// Milliseconds since the Unix epoch.
    pub fn unix_millis(&self) -> i64 {
        self.0.unix_timestamp_nanos().div_euclid(1_000_000) as i64
    }

    /// The underlying instant (UTC).
    pub fn as_datetime(&self) -> OffsetDateTime {
        self.0
    }

    /// Parses any RFC 3339 timestamp and normalises it to UTC.
    /// Returns an error if the normalised UTC instant is outside the supported range.
    pub fn parse(s: &str) -> Result<Self, time::error::Parse> {
        let instant = OffsetDateTime::parse(s, &Rfc3339)?;
        // Validate UTC representability before entering the infallible constructor.
        let utc = OffsetDateTime::from_unix_timestamp_nanos(instant.unix_timestamp_nanos())
            .map_err(time::error::TryFromParsed::from)?;
        Ok(Self::new(utc))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.0.format(WIRE_FORMAT).map_err(|_| fmt::Error)?;
        f.write_str(&s)
    }
}

impl From<OffsetDateTime> for Timestamp {
    fn from(value: OffsetDateTime) -> Self {
        Self::new(value)
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Timestamp {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Timestamp")
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("den_protocol::timestamp::Timestamp")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "date-time",
            "pattern": TIMESTAMP_PATTERN,
            "description": "RFC 3339 UTC instant with milliseconds and a `Z` suffix (Spec 02 §3)."
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_with_milliseconds_and_z() {
        let ts = Timestamp::parse("2026-10-18T22:15:03.120456+02:00").unwrap();
        assert_eq!(ts.to_string(), "2026-10-18T20:15:03.120Z");
        assert_eq!(
            serde_json::to_string(&ts).unwrap(),
            "\"2026-10-18T20:15:03.120Z\""
        );
        let back: Timestamp = serde_json::from_str("\"2026-10-18T20:15:03.120Z\"").unwrap();
        assert_eq!(back.unix_millis(), ts.unix_millis());
    }

    #[test]
    fn whole_seconds_still_carry_three_digits() {
        let ts = Timestamp::from_unix_millis(0).unwrap();
        assert_eq!(ts.to_string(), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn truncation_is_consistent_before_the_epoch() {
        let ts = Timestamp::parse("1969-12-31T23:59:59.9995Z").unwrap();
        assert_eq!(ts.to_string(), "1969-12-31T23:59:59.999Z");
        assert_eq!(ts.unix_millis(), -1);
        assert_eq!(
            Timestamp::parse(&ts.to_string()).unwrap(),
            ts,
            "a value equals its wire form"
        );
    }

    #[test]
    fn rejects_non_rfc3339() {
        assert!(serde_json::from_str::<Timestamp>("\"yesterday\"").is_err());
    }
}

#[cfg(test)]
mod utc_range_regressions {
    use super::*;

    #[test]
    fn utc_overflow_is_a_parse_error_at_both_ingresses() {
        for text in [
            "9999-12-31T23:00:00-01:00",
            "9999-12-31T23:00:00.000000001-01:00",
            "9999-12-31T23:59:59.999999999999-00:01",
            "9999-12-31T00:01:00-23:59",
            "9999-12-31T23:59:60-00:01",
        ] {
            let parsed = Timestamp::parse(text);
            assert!(
                matches!(
                    parsed,
                    Err(time::error::Parse::TryFromParsed(
                        time::error::TryFromParsed::ComponentRange(_)
                    ))
                ),
                "{text}: {parsed:?}"
            );
            let json = serde_json::to_string(text).unwrap();
            let error = serde_json::from_str::<Timestamp>(&json).unwrap_err();
            assert!(error.is_data(), "{text}: {error}");
        }
    }

    #[test]
    fn supported_boundaries_and_existing_normalization_are_preserved() {
        for (input, wire) in [
            ("9999-12-31T22:59:59-01:00", "9999-12-31T23:59:59.000Z"),
            (
                "9999-12-31T22:59:59.999999999999-01:00",
                "9999-12-31T23:59:59.999Z",
            ),
            ("9999-12-31T23:59:59.999999999Z", "9999-12-31T23:59:59.999Z"),
            ("9999-12-31T23:59:60Z", "9999-12-31T23:59:59.999Z"),
            (
                "9999-12-31T23:59:60.123456789012Z",
                "9999-12-31T23:59:59.999Z",
            ),
            (
                "2026-10-31T23:59:60.123456789012Z",
                "2026-10-31T23:59:59.999Z",
            ),
            ("2026-11-01T00:59:60+01:00", "2026-10-31T23:59:59.999Z"),
            ("1969-12-31T23:59:59.9995Z", "1969-12-31T23:59:59.999Z"),
            (
                "2026-10-18T22:15:03.120456+02:00",
                "2026-10-18T20:15:03.120Z",
            ),
            ("0000-01-01T00:00:00Z", "0000-01-01T00:00:00.000Z"),
        ] {
            let parsed = Timestamp::parse(input).unwrap();
            assert_eq!(parsed.to_string(), wire, "{input}");
            let json = serde_json::to_string(input).unwrap();
            assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), parsed);
            assert_eq!(
                serde_json::to_string(&parsed).unwrap(),
                format!("\"{wire}\"")
            );
            assert_eq!(Timestamp::parse(wire).unwrap(), parsed);
        }
        // Year -1 UTC is already accepted; do not impose a new four-digit range policy here.
        let lower = Timestamp::parse("0000-01-01T00:00:00+23:59").unwrap();
        assert_eq!(lower.as_datetime().year(), -1);
        assert_eq!(lower.as_datetime().offset(), UtcOffset::UTC);
        for separator in ['T', 't', ' ', 'X', '_', '\n', '0'] {
            let text = format!("2026-10-20{separator}00:00:00z");
            assert_eq!(
                Timestamp::parse(&text).unwrap().to_string(),
                "2026-10-20T00:00:00.000Z"
            );
        }
    }

    #[test]
    fn malformed_unicode_and_control_strings_return_errors_without_panicking() {
        for text in [
            "",
            "yesterday",
            "💥",
            "2026-10-20💥00:00:00Z",
            "\0",
            "9999-12-31T23:00:00-01:💥",
            "2026-10-30T23:59:60Z",
            "10000-01-01T00:00:00Z",
            "2026-10-20T00:00:00+24:00",
            "9999-12-31T23:00:00-01:00trailing",
        ] {
            assert!(Timestamp::parse(text).is_err(), "{text:?}");
            assert!(
                serde_json::from_str::<Timestamp>(&serde_json::to_string(text).unwrap()).is_err()
            );
        }
        for replacement in ['\0', 'é', '中', '💥', '\u{7f}'] {
            for position in [0, 4, 5, 7, 8, 11, 13, 14, 16, 17, 19] {
                let mut text = String::from("2026-10-20T00:00:00Z");
                text.replace_range(position..position + 1, &replacement.to_string());
                assert!(Timestamp::parse(&text).is_err(), "{text:?}");
                assert!(
                    serde_json::from_str::<Timestamp>(&serde_json::to_string(&text).unwrap())
                        .is_err()
                );
            }
        }
    }
}
