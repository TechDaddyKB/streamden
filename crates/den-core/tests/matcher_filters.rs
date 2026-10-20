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
