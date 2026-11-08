//! Port of tests/test_rules.py's interface surface test.

use godot_guard::interface;

const WIDE_NODE: &str = include_str!("fixtures/wide_node.gd");

#[test]
fn counts_exports_signals_and_setup_parameters() {
    let surface = interface::parse("wide.gd", WIDE_NODE);
    assert_eq!(surface.exports, ["color", "size", "level", "late"]);
    assert_eq!(surface.signals, ["opened", "closed"]);
    let names = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        surface.parameters,
        [
            ("_init".to_owned(), names(&["a", "b"])),
            ("setup".to_owned(), names(&["first", "second", "third"]))
        ]
    );
    assert_eq!(surface.total(), 11);
}

fn exports_of(source: &str) -> Vec<String> {
    interface::parse("s.gd", source).exports
}

#[test]
fn an_export_annotation_on_the_same_line_or_a_line_above_exports_the_variable() {
    let source = concat!(
        "@export var inline\n",
        "@export\nvar above\n",
        "@export_range(0, 1)\n@onready\nvar stacked\n",
        "@onready @export var both\n",
        "@onready var node\n",
        "var plain\n",
    );
    assert_eq!(exports_of(source), ["inline", "above", "stacked", "both"]);
}

#[test]
fn group_annotations_and_the_next_variable_do_not_carry_an_export() {
    let source = concat!(
        "@export_group(\"Look\")\nvar first\n",
        "@export_subgroup(\"Sub\")\n@export_category(\"Cat\")\nvar second\n",
        "@export\n@export_group(\"Mid\")\nvar third\n",
    );
    assert_eq!(exports_of(source), ["third"]);
}

#[test]
fn an_export_waits_only_while_annotations_continue() {
    let source = "@export\nsignal gone\nvar first\n@export var second\n# @export\nvar third\n";
    assert_eq!(exports_of(source), ["second"]);
}

#[test]
fn indented_lines_and_functions_are_not_exports() {
    let source = "@export var a\nfunc f():\n\t@export var inner\n\tvar local\n@export\nstatic func g():\n\tpass\nvar b\n";
    assert_eq!(exports_of(source), ["a"]);
}
