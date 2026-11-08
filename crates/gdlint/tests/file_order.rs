//! gdlint lints files in parallel: its report must still follow the files' order and stop where a file
//! stops gdlint, a panic included.

use std::panic::{AssertUnwindSafe, catch_unwind};

use gdlint::cli::run;

/// One `function-name` problem.
const BAD_NAME: &str = "func BadName():\n\tpass\n";
/// Not UTF-8: gdlint stops at this file with a traceback.
const UNDECODABLE: &[u8] = b"var \xff = 1\n";

/// A fresh directory holding 40 files `f00.gd` to `f39.gd`, `code_of` giving each one's bytes.
fn project_of_40_files(
    name: &str,
    code_of: impl Fn(usize) -> Vec<u8>,
) -> (std::path::PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("gdlint-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let names: Vec<String> = (0..40).map(|i| format!("f{i:02}.gd")).collect();
    for (i, file) in names.iter().enumerate() {
        std::fs::write(dir.join(file), code_of(i)).unwrap();
    }
    (dir, names)
}

/// The file each line of the report names.
fn reported_files(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .map(|line| line.split(':').next().unwrap())
        .collect()
}

/// A parser that panics on a file holding a `# first panic` or `# second panic` marker.
fn parse_panicking_on_markers(code: &str) -> Result<gdscript::Tree, gdscript::ParseError> {
    if code.contains("# first panic") {
        panic!("first");
    }
    if code.contains("# second panic") {
        panic!("second");
    }
    gdscript::parse(code)
}

#[test]
fn reports_in_file_order_up_to_the_stop() {
    let (dir, names) = project_of_40_files("order", |i| match i {
        30 => UNDECODABLE.to_vec(),
        _ => BAD_NAME.as_bytes().to_vec(),
    });
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&names, &dir, &gdscript::parse, &mut out, &mut err).unwrap();
    let stderr = String::from_utf8(err).unwrap();
    let expected: Vec<&str> = names[..=30].iter().map(String::as_str).collect();
    assert_eq!(reported_files(&stderr), expected, "{stderr}");
    assert!(
        stderr
            .lines()
            .last()
            .unwrap()
            .contains("UnicodeDecodeError")
    );
    assert_eq!((out.as_slice(), status), (&b""[..], 1));
}

/// The checks panic where the Python crashes with a traceback. The files before the first such file
/// still report, then that file's panic is the one raised, whichever panic finishes first.
#[test]
fn a_panic_surfaces_after_the_reports_before_it() {
    let (dir, names) = project_of_40_files("panic", |i| match i {
        10 => b"# first panic\n".to_vec(),
        25 => b"# second panic\n".to_vec(),
        _ => BAD_NAME.as_bytes().to_vec(),
    });
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let result = catch_unwind(AssertUnwindSafe(|| {
        run(
            &names,
            &dir,
            &parse_panicking_on_markers,
            &mut out,
            &mut err,
        )
    }));
    let payload = result.expect_err("the panic reaches the caller");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"first"));
    let stderr = String::from_utf8(err).unwrap();
    let expected: Vec<&str> = names[..10].iter().map(String::as_str).collect();
    assert_eq!(reported_files(&stderr), expected, "{stderr}");
}

/// Nested parens deep enough to overflow a 2 MiB rayon worker stack but not the main thread's 8 MiB.
const NESTED_PARENS: usize = 40_000;

#[test]
fn deeply_nested_code_lints_without_overflowing_a_worker() {
    let dir = std::env::temp_dir().join(format!("gdlint-deep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let code = format!(
        "func f():\n\tvar x = {}1{}\n",
        "(".repeat(NESTED_PARENS),
        ")".repeat(NESTED_PARENS)
    );
    std::fs::write(dir.join("deep.gd"), code).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(
        &["deep.gd".to_owned()],
        &dir,
        &gdscript::parse,
        &mut out,
        &mut err,
    )
    .unwrap();
    let stderr = String::from_utf8(err).unwrap();
    let expected = "deep.gd:2: Error: Max allowed line length (100) exceeded (max-line-length)\nFailure: 1 problem found\n";
    assert_eq!(
        (out.as_slice(), stderr.as_str(), status),
        (&b""[..], expected, 1)
    );
}
