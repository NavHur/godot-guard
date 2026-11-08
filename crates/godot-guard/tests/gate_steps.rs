//! The steps a gate plans, by name and phase, for each depth and gate configuration.

mod common;

use std::path::Path;

use common::Repo;
use godot_guard::gate::{Depth, Plan, steps_for};
use godot_guard::schedule::Phase;
use godot_guard::scripts::Scripts;

fn planned(gate: &str, depth: Depth) -> Vec<(String, Phase)> {
    let repo = Repo::new(&[], Some(&format!("gate:\n{gate}")));
    let config = repo.config();
    let scripts = Scripts::new();
    let plan = Plan {
        config: &config,
        chosen: None,
        sources: &config,
        scripts: &scripts,
        analyzer: Path::new("/tmp/analyze_scripts.gd"),
    };
    let files = vec!["a.gd".to_owned()];
    steps_for(&plan, &files, depth)
        .into_iter()
        .map(|step| (step.name, step.phase))
        .collect()
}

fn step(name: &str, phase: Phase) -> (String, Phase) {
    (name.to_owned(), phase)
}

#[test]
fn the_default_full_gate_formats_lints_checks_imports_analyzes_and_smokes() {
    assert_eq!(
        planned("  jobs: 1\n", Depth::Full),
        [
            step("format", Phase::Prepare),
            step("lint", Phase::Prepare),
            step("check", Phase::Prepare),
            step("import", Phase::Import),
            step("analyzer", Phase::Test),
            step("smoke", Phase::Test),
        ]
    );
}

#[test]
fn the_fast_gate_stops_after_format_lint_and_check() {
    let gate = "  steps: [make]\n  test-commands: [pytest]\n  tests: [t.gd]\n";
    assert_eq!(
        planned(gate, Depth::Fast),
        [
            step("format", Phase::Prepare),
            step("lint", Phase::Prepare),
            step("check", Phase::Prepare),
        ]
    );
}

#[test]
fn disabled_tools_leave_only_the_check_and_shell_steps_run_in_their_phases() {
    let gate = concat!(
        "  format: false\n  lint: false\n  import: false\n  analyzer: false\n  smoke: false\n",
        "  steps: [make, lint-docs]\n  test-commands: [pytest]\n  tests: [t.gd]\n",
    );
    assert_eq!(
        planned(gate, Depth::Full),
        [
            step("check", Phase::Prepare),
            step("make", Phase::Prepare),
            step("lint-docs", Phase::Prepare),
            step("tests t.gd", Phase::Test),
            step("pytest", Phase::Test),
        ]
    );
}
