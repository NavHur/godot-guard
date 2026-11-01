//! gdlint lints files in parallel: its report must still follow the files' order and stop where a file
//! stops gdlint.

use gdlint::cli::run;

/// One `function-name` problem.
const BAD_NAME: &str = "func BadName():\n\tpass\n";
/// Not UTF-8: gdlint stops at this file with a traceback.
const UNDECODABLE: &[u8] = b"var \xff = 1\n";

#[test]
fn reports_in_file_order_up_to_the_stop() {
    let dir = std::env::temp_dir().join(format!("gdlint-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let names: Vec<String> = (0..40).map(|i| format!("f{i:02}.gd")).collect();
    for (i, name) in names.iter().enumerate() {
        let code = if i == 30 {
            UNDECODABLE
        } else {
            BAD_NAME.as_bytes()
        };
        std::fs::write(dir.join(name), code).unwrap();
    }
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&names, &dir, &gdscript::parse, &mut out, &mut err).unwrap();
    let stderr = String::from_utf8(err).unwrap();
    let reported: Vec<&str> = stderr
        .lines()
        .map(|line| line.split(':').next().unwrap())
        .collect();
    let expected: Vec<&str> = names[..=30].iter().map(String::as_str).collect();
    assert_eq!(reported, expected, "{stderr}");
    assert!(
        stderr
            .lines()
            .last()
            .unwrap()
            .contains("UnicodeDecodeError")
    );
    assert_eq!((out.as_slice(), status), (&b""[..], 1));
}
