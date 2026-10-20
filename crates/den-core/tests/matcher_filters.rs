use std::cmp::Ordering;

use den_core::matcher::filter::values;
use serde_json::{Value, json};

#[test]
fn normalized_json_equality_and_numeric_order() {
    for (a, b, order) in [
        (json!(1), json!(1.0), Ordering::Equal),
        (json!(-0.0), json!(0), Ordering::Equal),
        (json!(-0.0), json!(0.0), Ordering::Equal),
        (json!(1.5), json!(1.5), Ordering::Equal),
        (json!(-2), json!(-1.5), Ordering::Less),
        (json!(0), json!(0.001), Ordering::Less),
        (json!(100), json!(99.99), Ordering::Greater),
        (json!(1e100), json!(1e99), Ordering::Greater),
        (json!(i64::MIN), json!(u64::MAX), Ordering::Less),
        (json!(i64::MAX), json!(i64::MAX as u64 + 1), Ordering::Less),
        (json!(u64::MAX - 1), json!(u64::MAX), Ordering::Less),
        (
            json!(i64::MAX),
            json!(9223372036854775808.0),
            Ordering::Less,
        ),
        (
            json!(i64::MIN + 1),
            json!(-9223372036854775808.0),
            Ordering::Greater,
        ),
        (
            json!(-9007199254740993_i64),
            json!(-9007199254740992.0),
            Ordering::Less,
        ),
        (json!(1), json!(1.5), Ordering::Less),
        (json!(-1), json!(-1.5), Ordering::Greater),
        (json!(0), json!(-f64::from_bits(1)), Ordering::Greater),
        (json!(0), json!(f64::from_bits(1)), Ordering::Less),
        (json!(u64::MAX), json!(f64::MAX), Ordering::Less),
        (json!(i64::MIN), json!(-f64::MAX), Ordering::Greater),
        (
            json!(9007199254740993_u64),
            json!(9007199254740992.0),
            Ordering::Greater,
        ),
        (
            json!(u64::MAX),
            json!(18446744073709551616.0),
            Ordering::Less,
        ),
        (
            json!(i64::MIN),
            json!(-9223372036854775808.0),
            Ordering::Equal,
        ),
    ] {
        assert_eq!(values::compare(&a, &b), Some(order), "{a} / {b}");
        assert_eq!(values::compare(&b, &a), Some(order.reverse()));
        assert_eq!(values::equal(&a, &b, false), order.is_eq());
        assert_eq!(values::equal(&b, &a, true), order.is_eq());
    }
    assert!(values::equal(
        &json!({"x": [1, null]}),
        &json!({"x": [1.0, null]}),
        false
    ));
    assert!(!values::equal(&json!([1]), &json!(["1"]), false));
    assert!(!values::equal(&json!("Alpha"), &json!("alpha"), false));
    assert!(values::equal(&json!("Straße"), &json!("STRASSE"), true));
}

#[test]
fn rfc3339_comparison_preserves_offsets_and_fractional_precision() {
    for (a, b, order) in [
        (
            "2026-10-20T00:00:00Z",
            "2026-10-20T01:00:00+01:00",
            Ordering::Equal,
        ),
        // Preserve the existing parser's accepted space separator.
        (
            "2026-10-20 00:00:00.00001Z",
            "2026-10-20T00:00:00.000010Z",
            Ordering::Equal,
        ),
        (
            "2026-10-20T00:00:00.1Z",
            "2026-10-20T00:00:00.1000Z",
            Ordering::Equal,
        ),
        (
            "2026-10-20T00:00:00.0001Z",
            "2026-10-20T00:00:00.0002Z",
            Ordering::Less,
        ),
        (
            "1969-12-31T23:59:59.9991Z",
            "1969-12-31T23:59:59.9992Z",
            Ordering::Less,
        ),
        (
            "2026-10-20T00:00:00.1234567891Z",
            "2026-10-20T00:00:00.1234567892Z",
            Ordering::Less,
        ),
        (
            "2016-12-31T23:59:59.999Z",
            "2016-12-31T23:59:60Z",
            Ordering::Less,
        ),
        (
            "2016-12-31T23:59:60Z",
            "2017-01-01T00:00:00Z",
            Ordering::Less,
        ),
        (
            "1969-12-31T23:59:59.9999999999Z",
            "1970-01-01T00:00:00Z",
            Ordering::Less,
        ),
        (
            "2026-10-20t00:00:00.00000000001z",
            "2026-10-19T19:00:00.0000000000100-05:00",
            Ordering::Equal,
        ),
        (
            "2016-12-31T23:59:60.1Z",
            "2017-01-01T00:59:60.1000+01:00",
            Ordering::Equal,
        ),
        (
            "2016-12-31T23:59:60.1Z",
            "2016-12-31T23:59:60.2Z",
            Ordering::Less,
        ),
        (
            "2016-12-31T23:59:60.9999999999Z",
            "2017-01-01T00:00:00Z",
            Ordering::Less,
        ),
    ] {
        assert_eq!(
            values::compare(&json!(a), &json!(b)),
            Some(order),
            "{a} / {b}"
        );
        assert_eq!(values::compare(&json!(b), &json!(a)), Some(order.reverse()));
        assert!(den_protocol::Timestamp::parse(a).is_ok());
        assert!(den_protocol::Timestamp::parse(b).is_ok());
    }
    for invalid in [
        "yesterday",
        "2026-02-30T00:00:00Z",
        "2026-10-20",
        "1",
        "2026-10-20T24:00:00Z",
        "2026-10-20T00:00:00+24:00",
        "2026-10-20T00:00:00.Z",
        "2026-10-20T00:00:00Zextra",
        "2026-10-20T00:00:00",
        "é2026-10-20T00:00:00Z",
    ] {
        assert_eq!(values::compare(&json!(invalid), &json!(invalid)), None);
    }
    assert_eq!(values::compare(&json!(1), &json!("1")), None);
}

#[test]
fn recursive_equality_keeps_json_types_keys_and_array_order() {
    for (a, b, insensitive, expected) in [
        (
            json!({"x": [1, {"v": "Straße"}]}),
            json!({"x": [1.0, {"v": "STRASSE"}]}),
            true,
            true,
        ),
        (json!({"X": 1}), json!({"x": 1.0}), true, false),
        (json!([1, 2]), json!([2.0, 1.0]), true, false),
        (json!([1]), json!([1.0, 2]), false, false),
        (json!({"x": 1}), json!({"x": 1.0, "y": null}), false, false),
        (json!({"x": null}), json!({"y": null}), true, false),
        (json!("Σςσ"), json!("σσσ"), true, true),
        (json!("ﬃ"), json!("FFI"), true, true),
        (json!("Straße"), json!("STRASSE"), false, false),
        (json!("é"), json!("e\u{301}"), true, false),
    ] {
        assert_eq!(values::equal(&a, &b, insensitive), expected, "{a} / {b}");
        assert_eq!(values::equal(&b, &a, insensitive), expected);
    }
    let types = [
        Value::Null,
        json!(false),
        json!(0),
        json!("0"),
        json!([]),
        json!({}),
    ];
    for (i, a) in types.iter().enumerate() {
        for (j, b) in types.iter().enumerate() {
            assert_eq!(values::equal(a, b, true), i == j, "{a} / {b}");
            assert_eq!(
                values::compare(a, b),
                (i == 2 && j == 2).then_some(Ordering::Equal)
            );
        }
    }
    // Equality is JSON equality, rather than timestamp coercion.
    assert!(!values::equal(
        &json!("2026-10-20T00:00:00Z"),
        &json!("2026-10-20T01:00:00+01:00"),
        false
    ));
}

#[test]
fn timestamp_utc_boundaries_reject_overflow_without_narrowing_parser_inputs() {
    let last = "9999-12-31T23:59:59.999999999999Z";
    for invalid in [
        "9999-12-31T23:00:00-01:00",
        "9999-12-31T23:59:60-00:01",
        "9999-12-31T00:01:00-23:59",
        "9999-12-31_23:59:59.999999999999-00:01",
        "9999-12-31T23:59:59-24:00",
        "9999-12-31T23:59:59-00:60",
        "9999-12-31T23:59:59-99:99",
        "9999-12-31T23:59:59-01:00extra",
        "9999-12-31T23:59:59-é:00",
        "9999-12-31T23:59:59-00:é",
        "9999-12-31é23:59:59-01:00",
        "9999-12-31Té:59:59-01:00",
        "9999-12-31T23:é:59-01:00",
        "9999-12-31T23:59:é-01:00",
    ] {
        assert_eq!(
            values::compare(&json!(invalid), &json!(last)),
            None,
            "{invalid}"
        );
        assert_eq!(
            values::compare(&json!(last), &json!(invalid)),
            None,
            "{invalid}"
        );
        assert_eq!(values::compare(&json!(invalid), &json!(invalid)), None);
    }
    for (a, b, order) in [
        (
            "9999-12-31T22:59:59-01:00",
            "9999-12-31T23:59:59Z",
            Ordering::Equal,
        ),
        (
            "9999-12-31T00:00:59-23:59",
            "9999-12-31T23:59:59Z",
            Ordering::Equal,
        ),
        (
            "9999-12-31T22:59:60.1-01:00",
            "9999-12-31T23:59:60.1000Z",
            Ordering::Equal,
        ),
        (last, "9999-12-31T23:59:60Z", Ordering::Less),
        (
            "9999-12-31T23:59:60.1Z",
            "9999-12-31T23:59:60.2Z",
            Ordering::Less,
        ),
        (
            "0000-01-01T00:00:00+23:59",
            "0000-01-01T00:00:00Z",
            Ordering::Less,
        ),
    ] {
        assert!(den_protocol::Timestamp::parse(a).is_ok());
        assert!(den_protocol::Timestamp::parse(b).is_ok());
        assert_eq!(
            values::compare(&json!(a), &json!(b)),
            Some(order),
            "{a} / {b}"
        );
        assert_eq!(values::compare(&json!(b), &json!(a)), Some(order.reverse()));
    }
    // time 0.3.55 skips any single separator byte, including X, _, newline, and '-'.
    for separator in 0_u8..=127 {
        let a = format!("9999-12-31{}23:59:59Z", char::from(separator));
        let b = format!("9999-12-31{}22:59:59-01:00", char::from(separator));
        for value in [a, b] {
            assert!(den_protocol::Timestamp::parse(&value).is_ok());
            assert_eq!(
                values::compare(&json!(value), &json!("9999-12-31T23:59:59Z")),
                Some(Ordering::Equal)
            );
        }
    }
    let fraction = "9".repeat(100_000);
    let valid = json!(format!("9999-12-31T22:59:59.{fraction}-01:00"));
    let overflow = json!(format!("9999-12-31T23:59:59.{fraction}-00:01"));
    assert_eq!(
        values::compare(&valid, &json!(last)),
        Some(Ordering::Greater)
    );
    assert_eq!(values::compare(&overflow, &valid), None);
    assert_eq!(values::compare(&valid, &overflow), None);
}
