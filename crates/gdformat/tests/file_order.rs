//! gdformat formats `--check` files in parallel: its report must still follow the files' order and stop
//! where a file stops gdformat, and formatting in place must still see each earlier write.

use std::path::{Path, PathBuf};

use gdformat::cli::{Io, run};

const UNFORMATTED: &str = "var x=1\n";
const FORMATTED: &str = "var x = 1\n";
/// Not UTF-8: gdformat stops at this file with a traceback.
const UNDECODABLE: &[u8] = b"var \xff = 1\n";

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gdformat-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn gdformat(args: &[String], cwd: &Path) -> (String, String, i32) {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut io = Io {
        stdin: &mut std::io::empty(),
        stdout: &mut stdout,
        stderr: &mut stderr,
    };
    let status = run(args, cwd, &mut io).unwrap();
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).unwrap();
    (text(stdout), text(stderr), status)
}

#[test]
fn check_reports_in_file_order_up_to_the_stop() {
    let dir = project("order");
    let names: Vec<String> = (0..40).map(|i| format!("f{i:02}.gd")).collect();
    for (i, name) in names.iter().enumerate() {
        let code: &[u8] = match i {
            30 => UNDECODABLE,
            _ if i % 3 == 0 => UNFORMATTED.as_bytes(),
            _ => FORMATTED.as_bytes(),
        };
        std::fs::write(dir.join(name), code).unwrap();
    }
    let mut args = vec!["--check".to_owned()];
    args.extend(names.iter().cloned());
    let (stdout, stderr, status) = gdformat(&args, &dir);
    let mut expected: String = (0..30)
        .step_by(3)
        .map(|i| format!("would reformat f{i:02}.gd\n"))
        .collect();
    expected.push_str("f30.gd: UnicodeDecodeError: 'utf-8' codec can't decode the file: ");
    assert!(stderr.starts_with(&expected), "{stderr}");
    assert_eq!(stderr.lines().count(), 11, "{stderr}");
    assert_eq!((stdout.as_str(), status), ("", 1));
}

/// Parses, then panics in the formatter: "no handler for pattern".
const PANICS_IN_MATCH: &str = "func f(x):\n\tmatch x:\n\t\t1, 2, 3:\n";
/// Parses, then panics in the formatter: "child 1 of property_custom_getter out of range".
const PANICS_IN_GETTER: &str = "var a: int = 0:\n\tset(value):\n\t\ta = value\n\tget:\n";

#[test]
fn a_panic_surfaces_after_the_reports_before_it() {
    let dir = project("panic");
    let names: Vec<String> = (0..40).map(|i| format!("f{i:02}.gd")).collect();
    for (i, name) in names.iter().enumerate() {
        let code = match i {
            10 => PANICS_IN_MATCH,
            25 => PANICS_IN_GETTER,
            _ if i % 3 == 0 => UNFORMATTED,
            _ => FORMATTED,
        };
        std::fs::write(dir.join(name), code).unwrap();
    }
    let mut args = vec!["--check".to_owned()];
    args.extend(names.iter().cloned());
    let (stdout, stderr, status) = gdformat(&args, &dir);
    let mut expected: String = (0..10)
        .step_by(3)
        .map(|i| format!("would reformat f{i:02}.gd\n"))
        .collect();
    expected
        .push_str("Traceback: gdformat crashed: format_func_statement: no handler for pattern\n");
    assert_eq!(stderr, expected);
    assert_eq!((stdout.as_str(), status), ("", 1));
}

#[test]
fn in_place_a_file_named_twice_is_reformatted_once() {
    let dir = project("twice");
    std::fs::write(dir.join("a.gd"), UNFORMATTED).unwrap();
    let args = ["a.gd".to_owned(), "a.gd".to_owned()];
    let (stdout, _, status) = gdformat(&args, &dir);
    assert_eq!(
        stdout,
        "reformatted a.gd\n1 file reformatted, 1 file left unchanged.\n"
    );
    assert_eq!(status, 0);
    assert_eq!(
        std::fs::read_to_string(dir.join("a.gd")).unwrap(),
        FORMATTED
    );
}

/// Between the 2 MiB worker stack (overflows from about 11,000) and the 8 MiB one (from about 45,000).
const NESTED_PARENS: usize = 25_000;

#[test]
fn deeply_nested_code_checks_without_overflowing_a_worker() {
    let dir = project("deep");
    let code = format!(
        "func f():\n\tvar x = {}1{}\n",
        "(".repeat(NESTED_PARENS),
        ")".repeat(NESTED_PARENS)
    );
    std::fs::write(dir.join("deep.gd"), code).unwrap();
    let (stdout, stderr, status) = gdformat(&["--check".to_owned(), "deep.gd".to_owned()], &dir);
    assert!(stderr.starts_with("would reformat deep.gd\n"), "{stderr}");
    assert_eq!((stdout.as_str(), status), ("", 1));
}
