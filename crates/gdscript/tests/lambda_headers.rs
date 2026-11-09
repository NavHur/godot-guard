//! Which newlines inside brackets open a multiline lambda body: the indenter looks back from the newline
//! for `func [NAME] ( ... ) [-> TYPE_HINT] :`. A body only parses when that lookback accepts the header.

use gdscript::{ErrorKind, ParseError};

/// Two statements, which only parse as a body the indenter opened: without it they share a line.
fn call_with(lambda: &str) -> Result<(), ParseError> {
    gdscript::parse(&format!(
        "func f():\n\tcall({lambda}\n\t\tpass\n\t\treturn 1\n\t)\n"
    ))
    .map(drop)
}

#[test]
fn every_header_shape_opens_a_body() {
    for header in [
        "func():",
        "func named():",
        "func(a, b):",
        "func(a) -> int:",
        "func named(a: int) -> Array[int]:",
        "func(a = (1), b = foo(2)):",
        "func(a = [(1)]):",
    ] {
        assert!(call_with(header).is_ok(), "{header}");
    }
}

#[test]
fn a_header_without_its_colon_or_func_opens_nothing() {
    for header in ["foo(a):", "(a):", "func(a)", "func a, b:"] {
        assert!(call_with(header).is_err(), "{header}");
    }
}

#[test]
fn an_unclosed_parameter_list_is_no_header() {
    assert!(call_with("func(a = (1:").is_err());
}

#[test]
fn a_body_stays_open_until_a_bracket_or_comma_closes_it() {
    let nested = "func f():\n\tcall(func():\n\t\tpass, func(a):\n\t\t\tpass)\n";
    assert!(gdscript::parse(nested).is_ok());
    let unclosed = "func f():\n\tcall(func():\n\t\tpass\n\t\t\tpass)\n";
    assert!(gdscript::parse(unclosed).is_err());
}

#[test]
fn the_indenter_reports_the_error_after_the_tokens_before_it() {
    // The newline after `=` queues tokens the parser rejects before it reaches the dedent error.
    let error = gdscript::parse("func f():\n\tvar a =\n  pass\n").unwrap_err();
    assert_eq!(error.line, Some(2), "{error}");
    assert!(
        matches!(error.kind, ErrorKind::UnexpectedToken { .. }),
        "{error}"
    );
    let dedent = gdscript::parse("func f():\n\t\tpass\n  pass\n").unwrap_err();
    assert!(dedent.to_string().contains("dedent"), "{dedent}");
}
