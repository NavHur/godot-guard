//! `gdlint::cli::run` on small projects: the verbose config log, the name checks, `duplicated-load`, and
//! the class order check over annotations. The fixture corpus does not pin the `-v` text, the scopes of
//! the name checks together, or which duplicate loads are reported.

use std::path::{Path, PathBuf};

use gdlint::cli::run;

struct Ran {
    stdout: String,
    stderr: String,
    status: i32,
}

/// A fresh project directory holding `files`, with no config above it.
fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gdlint-runs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, code) in files {
        std::fs::write(dir.join(file), code).unwrap();
    }
    dir
}

fn lint(dir: &Path, args: &[&str]) -> Ran {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&args, dir, &gdscript::parse, &mut out, &mut err).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    Ran {
        stdout: String::from_utf8(out).unwrap(),
        stderr: String::from_utf8(err).unwrap(),
        status,
    }
}

const LOADS: &str = "const A = preload(\"res://a.gd\")\n\
    const B = preload(\"res://a.gd\")\n\
    var c = load(\"res://b.gd\")\n\
    var d = load(\"res://b.gd\")\n\
    var e = load(path)\n\
    var f = foo(\"res://a.gd\")\n\
    var g = load(\"res://c.gd\")\n\
    func h():\n\
    \tvar x = preload(\"res://c.gd\")\n\
    \tload(\"res://c.gd\")\n\
    \tload(\"res://c.gd\")\n";

#[test]
fn only_load_and_preload_of_a_string_count_as_duplicate_loads() {
    let dir = project("loads", &[("loads.gd", LOADS)]);
    let ran = lint(&dir, &["loads.gd"]);
    assert_eq!(ran.stdout, "");
    assert_eq!(
        ran.stderr,
        "loads.gd:9: Error: Function-scope preload variable name \"x\" is not valid (function-preload-variable-name)\n\
         loads.gd:2: Error: duplicated loading of \"res://a.gd\" (duplicated-load)\n\
         loads.gd:4: Error: duplicated loading of \"res://b.gd\" (duplicated-load)\n\
         loads.gd:9: Error: duplicated loading of \"res://c.gd\" (duplicated-load)\n\
         loads.gd:10: Error: duplicated loading of \"res://c.gd\" (duplicated-load)\n\
         loads.gd:11: Error: duplicated loading of \"res://c.gd\" (duplicated-load)\n\
         Failure: 6 problems found\n"
    );
    assert_eq!(ran.status, 1);
}

const NAMES: &str = "class_name bad_class\n\
    signal BadSignal\n\
    const badConst = 1\n\
    const BadLoad = preload(\"res://a.gd\")\n\
    const bad_load = load(\"res://a.gd\")\n\
    var BadVar = 1\n\
    var BadLoadVar = preload(\"res://a.gd\")\n\
    class InnerOk:\n\
    \tvar Inner_Bad = 2\n\
    \tclass _deeper_bad:\n\
    \t\tfunc DeeperFunc(BadArg):\n\
    \t\t\tvar BadLocal = 1\n\
    \t\t\tvar good_preload = preload(\"res://a.gd\")\n\
    \t\t\tfor BadLoop in []:\n\
    \t\t\t\tpass\n\
    enum bad_enum { lower, UPPER }\n\
    func BadFunc(): pass\n";

const NAME_PROBLEMS: [&str; 20] = [
    "names.gd:11: Error: Function name \"DeeperFunc\" is not valid (function-name)",
    "names.gd:17: Error: Function name \"BadFunc\" is not valid (function-name)",
    "names.gd:10: Error: Class name \"_deeper_bad\" is not valid (sub-class-name)",
    "names.gd:1: Error: Class name \"bad_class\" is not valid (class-name)",
    "names.gd:2: Error: Signal name \"BadSignal\" is not valid (signal-name)",
    "names.gd:16: Error: Enum name \"bad_enum\" is not valid (enum-name)",
    "names.gd:16: Error: Enum element name \"lower\" is not valid (enum-element-name)",
    "names.gd:14: Error: Loop variable name \"BadLoop\" is not valid (loop-variable-name)",
    "names.gd:11: Error: Function argument name \"BadArg\" is not valid (function-argument-name)",
    "names.gd:12: Error: Function-scope variable name \"BadLocal\" is not valid (function-variable-name)",
    "names.gd:13: Error: Function-scope preload variable name \"good_preload\" is not valid (function-preload-variable-name)",
    "names.gd:3: Error: Constant name \"badConst\" is not valid (constant-name)",
    "names.gd:5: Error: Constant (load/preload) name \"bad_load\" is not valid (load-constant-name)",
    "names.gd:9: Error: Class-scope variable name \"Inner_Bad\" is not valid (class-variable-name)",
    "names.gd:6: Error: Class-scope variable name \"BadVar\" is not valid (class-variable-name)",
    "names.gd:16: Error: Definition out of order in global scope (class-definitions-order)",
    "names.gd:5: Error: duplicated loading of \"res://a.gd\" (duplicated-load)",
    "names.gd:7: Error: duplicated loading of \"res://a.gd\" (duplicated-load)",
    "names.gd:13: Error: duplicated loading of \"res://a.gd\" (duplicated-load)",
    "names.gd:11: Error: unused function argument 'BadArg' (unused-argument)",
];

#[test]
fn every_name_check_reads_its_own_scope_in_check_order() {
    let dir = project("names", &[("names.gd", NAMES)]);
    let ran = lint(&dir, &["names.gd"]);
    let expected = format!("{}\nFailure: 20 problems found\n", NAME_PROBLEMS.join("\n"));
    assert_eq!(ran.stderr, expected);
    assert_eq!((ran.stdout.as_str(), ran.status), ("", 1));
}

#[test]
fn a_disabled_check_reports_nothing_and_a_pattern_replaces_the_default() {
    let dir = project(
        "disabled",
        &[
            ("names.gd", NAMES),
            (
                "gdlintrc",
                "disable: [function-name, constant-name]\nclass-name: \"[a-z_]+\"\n",
            ),
        ],
    );
    let ran = lint(&dir, &["names.gd"]);
    let problems: Vec<&str> = ran.stderr.lines().collect();
    let kept: Vec<&str> = NAME_PROBLEMS
        .into_iter()
        .filter(|line| !line.ends_with("(function-name)") && !line.ends_with("(constant-name)"))
        .filter(|line| !line.ends_with("(class-name)"))
        .collect();
    assert_eq!(problems[..16], kept[..]);
    assert_eq!(problems[16], "Failure: 16 problems found");
    assert_eq!(ran.status, 1);
}

#[test]
fn annotations_decide_the_section_of_a_class_variable() {
    let code = "@onready var a = 1\n@export var b = 2\n@export @onready var c = 3\n\
                var d = 4\n@onready\nvar e = 5\nsignal s\n";
    let dir = project("annotations", &[("anno.gd", code)]);
    let ran = lint(&dir, &["anno.gd"]);
    assert_eq!(
        ran.stderr,
        "anno.gd:2: Error: Definition out of order in global scope (class-definitions-order)\n\
         anno.gd:3: Error: Definition out of order in global scope (class-definitions-order)\n\
         anno.gd:4: Error: Definition out of order in global scope (class-definitions-order)\n\
         anno.gd:7: Error: Definition out of order in global scope (class-definitions-order)\n\
         Failure: 4 problems found\n"
    );
}

#[test]
fn verbose_logs_the_default_config_when_there_is_no_file() {
    let dir = project("verbose-default", &[("a.gd", "var a = 1\n")]);
    let ran = lint(&dir, &["-v", "a.gd"]);
    let log: Vec<&str> = ran.stdout.lines().collect();
    assert_eq!(
        log[..4],
        [
            "INFO:root:No 'gdlintrc' nor '.gdlintrc' found. Using default config...",
            "INFO:root:Loaded config:",
            "INFO:root:('disable', [])",
            "INFO:root:('function-name', '(_on_([A-Z][a-z0-9]*)+(_[a-z0-9]+)*|_?[a-z][a-z0-9]*(_[a-z0-9]+)*)')",
        ]
    );
    assert_eq!(log.len(), 35, "{log:?}");
    assert_eq!(log[34], "Success: no problems found");
    assert_eq!((ran.stderr.as_str(), ran.status), ("", 0));
}

#[test]
fn verbose_logs_the_file_then_each_default_it_lacks() {
    let dir = project(
        "verbose-file",
        &[("a.gd", "var a = 1\n"), ("gdlintrc", "max-returns: 2\n")],
    );
    let shown = dir.join("gdlintrc").display().to_string();
    let ran = lint(&dir, &["-v", "a.gd"]);
    let log: Vec<&str> = ran.stdout.lines().collect();
    assert_eq!(
        log[..4],
        [
            format!("INFO:root:Config file found: '{shown}'"),
            "INFO:root:Loaded config:".to_owned(),
            "INFO:root:('max-returns', 2)".to_owned(),
            "INFO:root:Adding missing entry from defaults: ('disable', [])".to_owned(),
        ]
    );
    let missing = log
        .iter()
        .filter(|line| line.contains("Adding missing"))
        .count();
    assert_eq!(missing, 31);
    assert!(!log.iter().any(|line| line.contains("'max-returns', 6")));
    assert_eq!(ran.status, 0);
}
