//! The strict loader against PyYAML: YAML 1.1 scalars, and the errors and lines `StrictLoader` reports.

use godot_guard::yaml::{Value, load};

fn value(text: &str) -> Value {
    load(text).unwrap().unwrap()
}

fn scalar_of(text: &str) -> Value {
    match value(&format!("k: {text}\n")) {
        Value::Map(entries) => entries[0].1.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn scalars_resolve_as_yaml_1_1() {
    assert_eq!(scalar_of("off"), Value::Bool(false));
    assert_eq!(scalar_of("Yes"), Value::Bool(true));
    assert_eq!(scalar_of("'off'"), Value::Str("off".into()));
    assert_eq!(scalar_of("1_000"), Value::Int(1000));
    assert_eq!(scalar_of("017"), Value::Int(15));
    assert_eq!(scalar_of("0x1F"), Value::Int(31));
    assert_eq!(scalar_of("1:30"), Value::Int(90));
    assert_eq!(scalar_of("1.5"), Value::Float(1.5));
    assert_eq!(scalar_of("~"), Value::Null);
    assert_eq!(scalar_of(""), Value::Null);
    assert_eq!(scalar_of("auto"), Value::Str("auto".into()));
    assert_eq!(scalar_of("!!str 3"), Value::Str("3".into()));
    assert_eq!(
        scalar_of("2024-01-02"),
        Value::Timestamp("2024-01-02".into())
    );
}

#[test]
fn empty_stream_is_none() {
    assert_eq!(load("").unwrap(), None);
    assert_eq!(load("# only a comment\n").unwrap(), None);
}

#[test]
fn repr_matches_python() {
    assert_eq!(
        scalar_of("[a, 1, true, ~, 1.0, 1e20, 1.0e+20]").py_repr(),
        "['a', 1, True, None, 1.0, '1e20', 1e+20]"
    );
    assert_eq!(scalar_of("{a: 2}").py_str(), "{'a': 2}");
}

#[test]
fn strict_errors_point_at_their_line() {
    let cases = [
        ("a: 1\na: 2\n", "line 2: duplicate key 'a'"),
        ("1: a\ntrue: b\n", "line 2: duplicate key True"),
        (
            "a: 1\nb: {<<: {c: 1}}\n",
            "line 2: merge keys are not supported",
        ),
        ("a: &x 1\n", "line 1: anchors and aliases are not supported"),
        ("a: 1\n---\nb: 2\n", "line 2: but found another document"),
        (
            "a: !!python/none x\n",
            "line 1: could not determine a constructor for the tag 'tag:yaml.org,2002:python/none'",
        ),
        (
            "a: !custom x\n",
            "line 1: could not determine a constructor for the tag '!custom'",
        ),
        ("{[a]: 1}\n", "line 1: found unhashable key"),
    ];
    for (text, message) in cases {
        assert_eq!(load(text).unwrap_err().to_string(), message, "{text:?}");
    }
}

#[test]
fn syntax_errors_point_at_pyyamls_line() {
    let cases = [
        ("limits: {cognitive: 4\n", 2),
        ("a: [1, 2\n", 2),
        ("a:\n  - b\n c: 1\n", 3),
        ("a: \"x\n", 2),
    ];
    for (text, line) in cases
        .into_iter()
        .chain([("a: b: c\n", 1), ("- a\nb: 1\n", 2)])
    {
        assert_eq!(load(text).unwrap_err().line, Some(line), "{text:?}");
    }
}

fn int_of(text: &str) -> Result<Value, String> {
    match load(&format!("k: !!int {text}\n")) {
        Ok(Some(Value::Map(entries))) => Ok(entries[0].1.clone()),
        Ok(other) => panic!("{other:?}"),
        Err(error) => Err(error.to_string()),
    }
}

#[test]
fn ints_read_every_base_sign_and_separator() {
    let cases = [
        ("0", 0),
        ("-0", 0),
        ("0b101", 5),
        ("-0b101", -5),
        ("0x1f", 31),
        ("+0x10", 16),
        ("010", 8),
        ("1_000", 1000),
        ("0_7", 7),
        ("1:30", 90),
        ("-1:30:00", -5400),
        ("+-1", -1),
        ("9223372036854775807", i64::MAX),
    ];
    for (text, want) in cases {
        assert_eq!(int_of(text), Ok(Value::Int(want)), "{text}");
    }
}

#[test]
fn ints_that_python_rejects_or_that_overflow_are_errors() {
    for text in [
        "08",
        "0o17",
        "0xffffffffffffffff",
        "9223372036854775808",
        "-9223372036854775808",
        "1:99999999999999999999",
        "1:x",
    ] {
        assert_eq!(
            int_of(text),
            Err(format!("line 1: invalid int '{text}'")),
            "{text}"
        );
    }
}

#[test]
fn keys_that_compare_equal_in_python_are_duplicates() {
    let cases = [
        ("{1: 0, 1.0: 0}\n", "line 1: duplicate key 1.0"),
        ("{1: 0, true: 0}\n", "line 1: duplicate key True"),
        ("{a: 0, b: 0, a: 0}\n", "line 1: duplicate key 'a'"),
    ];
    for (text, message) in cases {
        assert_eq!(load(text).unwrap_err().to_string(), message, "{text:?}");
    }
    assert!(load("{'1': 0, 1: 0, a: 0, ~: 0}\n").is_ok());
}

#[test]
fn a_shallower_error_wins_over_an_earlier_deeper_one() {
    let text = "a: {b: {x: 1, x: 2}}\nc: {y: 1, y: 2}\n";
    assert_eq!(
        load(text).unwrap_err().to_string(),
        "line 2: duplicate key 'y'"
    );
}
