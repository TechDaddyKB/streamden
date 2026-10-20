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

use std::{borrow::Cow, collections::BTreeMap};

use den_protocol::{Event, Filter, FilterOp, FilterableField, rpc::ErrorCode};
use regex::{Regex, RegexBuilder};
use serde_json::Value;
use unicode_casefold::UnicodeCaseFold;

use crate::error::EngineError;

/// Owned captures; group zero is the whole match and unmatched groups remain absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captures {
    pub groups: Vec<Option<String>>,
    pub named: BTreeMap<String, String>,
}

/// Per-filter result; the index decides which trigger's captures survive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub pass: bool,
    pub captures: Option<Captures>,
}

/// A validated filter with its path, specificity and regex prepared once.
#[derive(Debug)]
pub struct CompiledFilter {
    filter: Filter,
    path: Vec<String>,
    insensitive: bool,
    regex: Option<Regex>,
    specificity: u64,
}

impl CompiledFilter {
    /// Compile without retaining malformed patterns in an error or panicking.
    pub fn compile(filter: &Filter, field_def: &FilterableField) -> Result<Self, EngineError> {
        let invalid = || EngineError::new(ErrorCode(ErrorCode::INVALID_PARAMS.to_owned()));
        if filter.field != field_def.field
            || !field_def.ops.contains(&filter.op)
            || filter.field.split('.').any(str::is_empty)
        {
            return Err(invalid());
        }
        let regex = if filter.op == FilterOp::Regex {
            Some(
                RegexBuilder::new(filter.value.as_str().ok_or_else(invalid)?)
                    .size_limit(1024 * 1024)
                    .build()
                    .map_err(|_| invalid())?,
            )
        } else {
            None
        };
        let valid = match filter.op {
            FilterOp::In | FilterOp::NotIn => filter.value.is_array(),
            FilterOp::Exists => filter.value.is_boolean(),
            FilterOp::Between => filter.value.as_array().is_some_and(|bounds| {
                bounds.len() == 2 && values::compare(&bounds[0], &bounds[1]).is_some()
            }),
            FilterOp::Gte | FilterOp::Lte => {
                values::compare(&filter.value, &filter.value).is_some()
            }
            _ => true,
        };
        if !valid {
            return Err(invalid());
        }
        let weight = match filter.op {
            FilterOp::Eq | FilterOp::In => 3,
            FilterOp::Ne | FilterOp::NotIn | FilterOp::Exists => 1,
            _ => 2,
        };
        Ok(Self {
            filter: filter.clone(),
            path: filter.field.split('.').map(str::to_owned).collect(),
            insensitive: field_def.case_insensitive,
            regex,
            specificity: u64::from(field_def.specificity) * weight,
        })
    }

    pub fn specificity(&self) -> u64 {
        self.specificity
    }

    fn lookup<'a>(&self, event: &'a Event) -> Option<Cow<'a, Value>> {
        fn descend<'a>(mut value: &'a Value, path: &[String]) -> Option<&'a Value> {
            for part in path {
                value = value.as_object()?.get(part)?;
            }
            Some(value)
        }
        match self.path.first()?.as_str() {
            "platform" => {
                if self.path.len() != 1 {
                    return None;
                }
                Some(Cow::Owned(Value::String(
                    event.platform.as_ref()?.to_string(),
                )))
            }
            "user" if self.path.len() > 1 => {
                let user = serde_json::to_value(event.user.as_ref()?).ok()?;
                Some(Cow::Owned(descend(&user, &self.path[1..])?.clone()))
            }
            _ => descend(&event.payload, &self.path).map(Cow::Borrowed),
        }
    }

    pub fn eval(&self, event: &Event) -> Outcome {
        let found = self.lookup(event);
        let mut outcome = Outcome {
            pass: false,
            captures: None,
        };
        if self.filter.op == FilterOp::Exists {
            outcome.pass =
                found.as_deref().is_some_and(|v| !v.is_null()) == (self.filter.value == true);
            return outcome;
        }
        let Some(value) = found.as_deref() else {
            return outcome;
        };
        let operand = &self.filter.value;
        let equal = |a: &Value, b: &Value| values::equal(a, b, self.insensitive);
        outcome.pass = match self.filter.op {
            FilterOp::Eq => equal(value, operand),
            FilterOp::Ne => !equal(value, operand),
            FilterOp::In | FilterOp::NotIn => {
                let member = operand
                    .as_array()
                    .is_some_and(|items| items.iter().any(|v| equal(value, v)));
                member == (self.filter.op == FilterOp::In)
            }
            FilterOp::Gte => values::compare(value, operand).is_some_and(|o| o.is_ge()),
            FilterOp::Lte => values::compare(value, operand).is_some_and(|o| o.is_le()),
            FilterOp::Between => operand.as_array().is_some_and(|bounds| {
                values::compare(value, &bounds[0]).is_some_and(|o| o.is_ge())
                    && values::compare(value, &bounds[1]).is_some_and(|o| o.is_le())
            }),
            FilterOp::Contains => match (value, operand) {
                (Value::String(text), Value::String(needle)) => text
                    .case_fold()
                    .collect::<String>()
                    .contains(&needle.case_fold().collect::<String>()),
                (Value::Array(items), _) => items.iter().any(|v| equal(v, operand)),
                _ => false,
            },
            FilterOp::Regex => {
                let matched = value
                    .as_str()
                    .and_then(|v| self.regex.as_ref()?.captures(v));
                if let Some(matched) = matched {
                    outcome.captures = Some(Captures {
                        groups: matched
                            .iter()
                            .map(|m| m.map(|m| m.as_str().to_owned()))
                            .collect(),
                        named: self
                            .regex
                            .as_ref()
                            .into_iter()
                            .flat_map(|r| r.capture_names().flatten())
                            .filter_map(|name| {
                                matched
                                    .name(name)
                                    .map(|m| (name.to_owned(), m.as_str().to_owned()))
                            })
                            .collect(),
                    });
                    true
                } else {
                    false
                }
            }
            FilterOp::Exists => unreachable!("handled above"),
        };
        outcome
    }
}
