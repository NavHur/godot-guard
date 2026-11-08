//! Port of tests/test_config.py: loading, defaults, and every validation error.

mod common;

use common::Repo;
use godot_guard::config::{self, Jobs, Level, TestScript};

fn repo(files: &[(&str, &str)], text: Option<&str>) -> Repo {
    Repo::new(files, text)
}

fn error_of(text: &str) -> String {
    repo(&[], Some(text)).config_error()
}

#[test]
fn defaults_without_a_config_file() {
    let config = repo(&[], None).config();
    assert_eq!(config.paths, ["."]);
    assert_eq!(config.limits.cognitive, 8);
    assert_eq!(config.gate.smoke, ["--quit-after", "3"]);
    assert_eq!(config.level("nested-ternary"), Level::Warn);
}

#[test]
fn partial_sections_merge_with_defaults() {
    let config = repo(
        &[],
        Some("limits:\n  cognitive: 6\ngate:\n  smoke: false\n"),
    )
    .config();
    assert_eq!(config.limits.cognitive, 6);
    assert_eq!(config.limits.cyclomatic, 16);
    assert!(config.gate.smoke.is_empty());
}

#[test]
fn unknown_keys_are_errors() {
    let cases = [
        ("limit:\n  cognitive: 6\n", "'limit'"),
        ("limits:\n  cognitve: 6\n", "'limits.cognitve'"),
        ("limits:\n  file-lines: 300\n", "'limits.file-lines'"),
        ("rules:\n  file-lines: off\n", "'rules.file-lines'"),
    ];
    for (text, key) in cases {
        assert!(error_of(text).contains(key), "{text:?}");
    }
}

#[test]
fn wrong_shapes_are_errors() {
    let cases = [
        ("false\n", "the file must be a mapping"),
        ("[]\n", "the file must be a mapping"),
        ("limits: false\n", "limits must be a mapping"),
        ("rules: []\n", "rules must be a mapping"),
        ("gate: 0\n", "gate must be a mapping"),
        ("boundaries: false\n", "boundaries must be a list"),
        ("paths: false\n", "paths must be a list of strings"),
        ("exclude: {}\n", "exclude must be a list of strings"),
    ];
    for (text, message) in cases {
        assert!(
            error_of(text).contains(message),
            "{text:?}: {}",
            error_of(text)
        );
    }
}

#[test]
fn null_takes_the_default() {
    let text =
        "paths: null\nlimits: ~\nrules:\n  long-comment: null\ngate:\n  smoke: null\nboundaries:\n";
    let config = repo(&[], Some(text)).config();
    assert_eq!(
        (config.paths.clone(), config.limits.cognitive),
        (vec![".".to_owned()], 8)
    );
    assert_eq!(config.level("long-comment"), Level::Error);
    assert_eq!(config.gate.smoke, ["--quit-after", "3"]);
    assert!(config.boundaries.is_empty());
    assert_eq!(repo(&[], Some("")).config().paths, ["."]);
}

#[test]
fn duplicate_keys_are_errors() {
    let cases = [
        (
            "limits: {cognitve: 1}\nlimits: {cognitive: 100}\n",
            "duplicate key 'limits'",
            "line 2",
        ),
        (
            "gate:\n  format: true\n  lint: true\n  format: false\n",
            "duplicate key 'format'",
            "line 4",
        ),
        (
            "boundaries:\n  - name: a\n    forbid: [x]\n    name: b\n",
            "duplicate key 'name'",
            "line 4",
        ),
    ];
    for (text, message, line) in cases {
        assert_eq!(
            error_of(text),
            format!(".godot-guard.yml: {line}: {message}")
        );
    }
}

#[test]
fn merge_keys_anchors_and_aliases_are_errors() {
    let cases = [
        (
            "limits:\n  cyclomatic: 16\n  <<: {cognitive: 100}\n",
            "line 3: merge keys are not supported",
        ),
        (
            "gate: &fast {lint: false}\n",
            "line 1: anchors and aliases are not supported",
        ),
        (
            "limits: {cognitive: 4}\nrules: {long-comment: *a}\n",
            "line 2: anchors and aliases are not supported",
        ),
        (
            "limits: &a {<<: *a}\n",
            "line 1: anchors and aliases are not supported",
        ),
    ];
    for (text, message) in cases {
        assert_eq!(error_of(text), format!(".godot-guard.yml: {message}"));
    }
}

#[test]
fn yaml_syntax_errors_are_one_line() {
    let message = error_of("limits: {cognitive: 4\n");
    assert!(
        message.starts_with(".godot-guard.yml: line 2: "),
        "{message}"
    );
    assert!(!message.contains('\n'));
}

#[test]
fn bad_level_is_an_error() {
    assert!(
        error_of("rules:\n  long-comment: fatal\n")
            .contains("rules.long-comment must be one of error, warn, off")
    );
    assert!(
        error_of("rules:\n  nested-ternary: error\n")
            .contains("rules.nested-ternary is advisory: use warn or off")
    );
}

#[test]
fn paths_and_exclude_select_scripts() {
    let files = [
        ("scripts/a.gd", ""),
        ("scripts/gen/b.gd", ""),
        ("addons/x/c.gd", ""),
        ("other/d.gd", ""),
        (".hidden/e.gd", ""),
    ];
    let chosen = repo(
        &files,
        Some("paths: [./scripts, addons]\nexclude: [scripts/gen]\n"),
    );
    assert_eq!(
        chosen.config().scanned_files(),
        ["addons/x/c.gd", "scripts/a.gd"]
    );
    assert_eq!(
        repo(&files, None).config().scanned_files(),
        ["other/d.gd", "scripts/a.gd", "scripts/gen/b.gd"]
    );
}

#[test]
fn root_is_found_from_a_subdirectory() {
    let repo = repo(&[("scripts/a.gd", "")], Some("paths: [scripts]\n"));
    assert_eq!(
        config::load(&repo.root.join("scripts")).unwrap().root,
        repo.root
    );
}

#[test]
fn gate_jobs_is_a_count_or_auto() {
    assert_eq!(repo(&[], None).config().gate.jobs, Jobs::Count(1));
    assert_eq!(
        repo(&[], Some("gate:\n  jobs: 4\n")).config().gate.jobs,
        Jobs::Count(4)
    );
    assert_eq!(
        repo(&[], Some("gate:\n  jobs: auto\n")).config().gate.jobs,
        Jobs::Auto
    );
}

#[test]
fn every_gate_setting_is_read() {
    let text = concat!(
        "gate:\n  format: false\n  lint: false\n  import: false\n  analyzer: false\n  jobs: 2\n",
        "  steps: [make]\n  tests: [t.gd]\n  test-commands: [pytest]\n  smoke: [--quit]\n",
    );
    let gate = repo(&[], Some(text)).config().gate;
    assert!(!(gate.format || gate.lint || gate.import || gate.analyzer));
    assert_eq!(gate.jobs, Jobs::Count(2));
    assert_eq!(gate.steps, ["make"]);
    assert_eq!(gate.test_commands, ["pytest"]);
    assert_eq!(gate.smoke, ["--quit"]);
    assert_eq!(gate.tests.len(), 1);
}

#[test]
fn a_null_gate_setting_keeps_its_default() {
    let gate = repo(&[], Some("gate: {format: ~, jobs: ~, steps: ~}\n"))
        .config()
        .gate;
    assert!(gate.format && gate.lint && gate.import && gate.analyzer);
    assert_eq!(gate.jobs, Jobs::Count(1));
    assert!(gate.steps.is_empty());
}

#[test]
fn badly_shaped_gate_settings_are_errors() {
    let cases = [
        ("format: 3", "gate.format must be true or false"),
        ("analyzer: auto", "gate.analyzer must be true or false"),
        ("steps: x", "gate.steps must be a list of strings"),
        ("steps: [1, true]", "gate.steps must be a list of strings"),
        ("tests: x", "gate.tests must be a list"),
        (
            "smoke: true",
            "gate.smoke must be a list of arguments or false",
        ),
        ("bogus: true", "unknown key 'gate.bogus'"),
    ];
    for (setting, message) in cases {
        let error = error_of(&format!("gate: {{{setting}}}\n"));
        assert_eq!(error, format!(".godot-guard.yml: {message}"), "{setting}");
    }
}

#[test]
fn bad_jobs_are_errors() {
    for value in ["0", "-1", "many", "true", "1.5", "'2'"] {
        let message = error_of(&format!("gate: {{jobs: {value}}}\n"));
        assert!(
            message.contains("gate.jobs must be a whole number of at least 1, or auto"),
            "{value}: {message}"
        );
    }
}

#[test]
fn test_entries_are_scripts_or_sharded_mappings() {
    let text = concat!(
        "gate:\n  tests:\n    - res://tests/a.gd\n    - script: res://tests/b.gd\n      shards: 3\n",
        "    - script: res://tests/c.gd\n    - script: res://tests/d.gd\n      shards: 1\n",
        "    - script: res://tests/e.gd\n      shards: null\n"
    );
    let script = |name: &str, shards| TestScript {
        script: format!("res://tests/{name}"),
        shards,
    };
    assert_eq!(
        repo(&[], Some(text)).config().gate.tests,
        [
            script("a.gd", None),
            script("b.gd", Some(3)),
            script("c.gd", None),
            script("d.gd", Some(1)),
            script("e.gd", None),
        ]
    );
}

#[test]
fn shards_one_is_kept_apart_from_no_shards() {
    let tests = repo(
        &[],
        Some("gate: {tests: [res://a.gd, {script: res://a.gd, shards: 1}]}\n"),
    )
    .config()
    .gate
    .tests;
    assert_eq!(tests[0].shards, None);
    assert_eq!(tests[1].shards, Some(1));
}

#[test]
fn bad_test_entries_are_errors() {
    let shards = "gate.tests shards must be a whole number of at least 1";
    let cases = [
        ("gate: {tests: res://a.gd}\n", "gate.tests must be a list"),
        (
            "gate: {tests: [3]}\n",
            "each gate.tests entry must be a string or a mapping",
        ),
        (
            "gate: {tests: [{script: res://a.gd, shard: 2}]}\n",
            "unknown key 'gate.tests.shard'",
        ),
        (
            "gate: {tests: [{shards: 2}]}\n",
            "gate.tests script must be a string",
        ),
        (
            "gate: {tests: [{script: 3}]}\n",
            "gate.tests script must be a string",
        ),
        ("gate: {tests: [{script: res://a.gd, shards: 0}]}\n", shards),
        (
            "gate: {tests: [{script: res://a.gd, shards: '2'}]}\n",
            shards,
        ),
        (
            "gate: {tests: [{script: res://a.gd, shards: true}]}\n",
            shards,
        ),
    ];
    for (text, message) in cases {
        assert!(
            error_of(text).contains(message),
            "{text:?}: {}",
            error_of(text)
        );
    }
}

#[test]
fn test_commands_are_a_list_of_strings() {
    assert!(repo(&[], None).config().gate.test_commands.is_empty());
    let text = "gate:\n  test-commands: ['sh a.sh', 'sh b.sh']\n";
    let config = repo(&[], Some(text)).config();
    assert_eq!(config.gate.test_commands, ["sh a.sh", "sh b.sh"]);
    for text in [
        "gate:\n  test-commands: sh a.sh\n",
        "gate:\n  test-commands: [sh a.sh, 1]\n",
    ] {
        assert_eq!(
            error_of(text),
            ".godot-guard.yml: gate.test-commands must be a list of strings",
            "{text:?}"
        );
    }
}

#[test]
fn smoke_arguments_become_strings() {
    let config = repo(&[], Some("gate: {smoke: [--quit-after, 3, true]}\n")).config();
    assert_eq!(config.gate.smoke, ["--quit-after", "3", "True"]);
    assert!(
        error_of("gate: {smoke: yes}\n")
            .contains("gate.smoke must be a list of arguments or false")
    );
}

#[test]
fn boundary_errors_name_the_boundary() {
    assert!(error_of("boundaries: [3]\n").contains("each boundary must be a mapping"));
    assert!(error_of("boundaries: [{nme: a}]\n").contains("unknown key 'boundaries.nme'"));
    assert!(
        error_of("boundaries: [{name: domain, forbid: ['(']}]\n")
            .contains("boundary domain: bad pattern '('")
    );
    assert!(
        error_of("boundaries: [{name: domain, paths: x}]\n")
            .contains("boundary domain paths must be a list of strings")
    );
}

#[test]
fn relaxed_warnings_must_be_required_ones() {
    let message = error_of("relaxed-warnings: [inferred_declaration]\n");
    assert!(message.contains(
        "relaxed-warnings: 'inferred_declaration' is not a warning analyzer-config requires"
    ));
    assert_eq!(
        repo(&[], Some("relaxed-warnings: [unsafe_cast]\n"))
            .config()
            .relaxed_warnings,
        ["unsafe_cast"]
    );
}

#[test]
fn the_walk_lists_files_first_then_sorted_subdirectories_skipping_hidden_and_symlinked() {
    let repo = repo(
        &[
            ("a.gd", ""),
            ("notes.txt", ""),
            ("z/c.gd", ""),
            ("m/d.gd", ""),
            ("m/n/e.gd", ""),
            (".hidden/f.gd", ""),
            ("m/.hidden/g.gd", ""),
            ("B/h.gd", ""),
        ],
        None,
    );
    std::os::unix::fs::symlink(repo.root.join("m"), repo.root.join("alias")).unwrap();
    let walk = |start: &str| config::walk_scripts(&repo.root, &repo.root.join(start));
    assert_eq!(walk(""), ["a.gd", "B/h.gd", "m/d.gd", "m/n/e.gd", "z/c.gd"]);
    assert_eq!(walk("m"), ["m/d.gd", "m/n/e.gd"]);
    assert_eq!(walk("a.gd"), ["a.gd"]);
    assert_eq!(walk("missing"), Vec::<String>::new());
    assert_eq!(walk("alias"), ["alias/d.gd", "alias/n/e.gd"]);
}
