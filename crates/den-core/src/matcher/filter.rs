//! Trigger filter comparison primitives (Spec 03 §6.2).

/// Internal, unfrozen building blocks for trigger filters; no trigger or index policy.
pub mod values {
    use std::cmp::Ordering;

    use den_protocol::Timestamp;
    use serde_json::Value;
    use unicode_casefold::UnicodeCaseFold;

    /// JSON equality with exact integer normalization and optional full Unicode case folding.
    /// Object keys remain exact; the case flag applies recursively to string values.
    pub fn equal(left: &Value, right: &Value, insensitive: bool) -> bool {
        match (left, right) {
            (Value::Number(_), Value::Number(_)) => compare(left, right) == Some(Ordering::Equal),
            (Value::String(a), Value::String(b)) if insensitive => a.case_fold().eq(b.case_fold()),
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b, insensitive))
            }
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(key, a)| b.get(key).is_some_and(|b| equal(a, b, insensitive)))
            }
            _ => left == right,
        }
    }

    fn integer(value: &Value) -> Option<i128> {
        let number = value.as_number()?;
        if number.is_f64() {
            return None;
        }
        number
            .as_i64()
            .map(i128::from)
            .or_else(|| number.as_u64().map(i128::from))
    }

    fn integer_float(integer: i128, float: f64) -> Ordering {
        // All serde_json stored integers (i64/u64) fit i128; huge finite floats saturate safely.
        integer.cmp(&(float as i128)).then_with(|| {
            if float.fract() > 0.0 {
                Ordering::Less
            } else if float.fract() < 0.0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
    }

    fn number_order(left: &Value, right: &Value) -> Option<Ordering> {
        match (integer(left), integer(right)) {
            (Some(a), Some(b)) => Some(a.cmp(&b)),
            (Some(a), None) => Some(integer_float(a, right.as_f64()?)),
            (None, Some(b)) => Some(integer_float(b, left.as_f64()?).reverse()),
            (None, None) => left.as_f64()?.partial_cmp(&right.as_f64()?),
        }
    }

    fn past_max_utc(value: &str) -> bool {
        // Until DEN-325 fixes Timestamp::parse, UTC conversion can panic above year 9999.
        // Only the final local date and a negative offset can exceed the upper UTC bound.
        let Some(rest) = value.strip_prefix("9999-12-31") else {
            return false;
        };
        // Search after seconds: even '-' is an accepted date/time separator.
        let Some((_, offset)) = rest.get(9..).and_then(|tail| tail.rsplit_once('-')) else {
            return false;
        };
        // Every slice is at most two bytes: malformed input cannot overflow this arithmetic.
        // Calendar, offset, and fractional syntax validation remains the existing parser's job.
        let n = |part: Option<&str>| part.and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        // The parser represents a leap second as second 59 with maximal nanoseconds.
        let local = n(rest.get(1..3)) * 3600 + n(rest.get(4..6)) * 60 + n(rest.get(7..9)).min(59);
        let offset = n(offset.get(..2)) * 3600 + n(offset.get(3..5)) * 60;
        local + offset >= 86_400
    }

    fn instant(value: &str) -> Option<(i64, bool, &str)> {
        if past_max_utc(value) {
            return None;
        }
        let timestamp = Timestamp::parse(value).ok()?;
        // The wire Timestamp truncates to milliseconds. Preserve the validated original fraction,
        // including more than nine digits, so distinct instants never collapse at that boundary.
        let fraction = value.get(19..)?.strip_prefix('.').map_or("", |tail| {
            let end = tail
                .find(|ch: char| !ch.is_ascii_digit())
                .unwrap_or(tail.len());
            tail[..end].trim_end_matches('0')
        });
        Some((
            timestamp.unix_millis().div_euclid(1000),
            value.as_bytes().get(17..19) == Some(b"60"),
            fraction,
        ))
    }

    /// Numeric or RFC 3339 instant ordering only; incompatible JSON types have no ordering.
    /// Timestamp validation follows the existing parser, without its wire precision loss.
    pub fn compare(left: &Value, right: &Value) -> Option<Ordering> {
        match (left, right) {
            (Value::Number(_), Value::Number(_)) => number_order(left, right),
            (Value::String(a), Value::String(b)) => Some(instant(a)?.cmp(&instant(b)?)),
            _ => None,
        }
    }
}
