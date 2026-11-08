//! The scoring cases that `tests/fixtures/extra.gd` and `nested_ternary.gd` once held in one function
//! each, kept as inline sources. The numbers are the oracle's (godot-guard 0.4.0 on gdtoolkit's trees) as
//! they stood in `tests/data/fixtures.jsonl`; the tree here comes from the Rust parser.

use gdrules::complexity::{nested_ternaries, score};
use gdrules::patterns::scan;

const WALK_AND_PROCESS: &str = r#"extends "res://base.gd"

var speed := 1.0:
	get():
		return speed if speed > 0 else 0.0


static func walk(items: Array, limit: int) -> int:
	var total := 0
	for item: int in items:
		while total < limit && item > 0 || not items:
			total += item
	match total:
		0 when limit > 1:
			return walk(items, limit - 1)
		_ when limit < 0:
			return self.walk(items, 0)
		1, 2:
			pass
		_:
			return -1
	return total


func _process(_delta: float) -> void:
	var d := {"a": 1}
	var n := Node.new()
	var h := func named(): return named()
	if d and n or h and d:
		pass
	elif not d:
		pass
	else:
		var s := $"Quoted/Path"
"#;

const PICK: &str = "extends Node


func pick(a: bool, b: bool) -> int:
	var flat := 1 if a else 2
	var chained := 1 if a else 2 if b else 3
	var wrapped := (1 if a else 2) if b else 3
	var in_condition := 1 if (a if b else false) else 2
	return flat + chained + wrapped + in_condition
";

type Row = (&'static str, usize, usize, Option<usize>, usize);

fn rows(source: &str) -> Vec<(String, usize, usize, Option<usize>, usize)> {
    let tree = gdscript::parse(source).expect("source parses");
    score(&tree)
        .into_iter()
        .map(|f| (f.name, f.line, f.end_line, f.cognitive, f.cyclomatic))
        .collect()
}

fn assert_rows(source: &str, expected: &[Row]) {
    let expected: Vec<_> = expected
        .iter()
        .map(|&(name, line, end, cognitive, cyclomatic)| {
            (name.to_owned(), line, end, cognitive, cyclomatic)
        })
        .collect();
    assert_eq!(rows(source), expected);
}

#[test]
fn match_guards_loops_and_boolean_runs_score_in_one_function() {
    assert_rows(
        WALK_AND_PROCESS,
        &[
            ("speed.get", 4, 5, Some(1), 2),
            ("walk", 8, 22, Some(9), 9),
            ("_process", 25, 34, Some(7), 6),
            ("_process.named", 28, 28, None, 1),
        ],
    );
}

#[test]
fn a_hot_function_reports_its_lookup_and_allocations() {
    let tree = gdscript::parse(WALK_AND_PROCESS).expect("source parses");
    let hits = scan(&tree);
    let lines = |rule: &str| -> Vec<usize> { hits[rule].iter().map(|(line, _)| *line).collect() };
    assert_eq!(lines("lookup-in-hot-path"), [34]);
    assert_eq!(lines("per-frame-allocation"), [26, 27]);
}

#[test]
fn four_ternary_shapes_score_in_one_function() {
    assert_rows(PICK, &[("pick", 4, 9, Some(9), 8)]);
    let tree = gdscript::parse(PICK).expect("source parses");
    assert_eq!(nested_ternaries(&tree), [6, 7, 8]);
}
