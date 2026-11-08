//! `gdformat::cli::run` on small inputs: what each path through the command prints, and the exit status.
//! These pin the behavior the fixture corpus does not reach: option short-circuits, config failures,
//! the `--check` summary, lambda headers, annotation placement, dot chains, and comment placement.

use std::path::Path;

use gdformat::cli::{Io, run};

struct Ran {
    stdout: String,
    stderr: String,
    status: i32,
}

fn run_in(cwd: &Path, args: &[&str], stdin: &str) -> Ran {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let status = run(
        &args,
        cwd,
        &mut Io {
            stdin: &mut stdin.as_bytes(),
            stdout: &mut stdout,
            stderr: &mut stderr,
        },
    )
    .unwrap();
    Ran {
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
        status,
    }
}

fn format_stdin(code: &str) -> String {
    let ran = run_in(Path::new("/"), &["-"], code);
    assert_eq!((ran.status, ran.stderr.as_str()), (0, ""), "{code}");
    ran.stdout
}

fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("gdformat-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn diff_is_refused_before_anything_is_read() {
    let ran = run_in(Path::new("/"), &["--diff", "missing.gd"], "");
    assert_eq!(ran.status, 2);
    assert_eq!(
        ran.stderr,
        "gdformat: --diff is not supported by this port\n"
    );
}

#[test]
fn the_default_config_is_written_once() {
    let dir = scratch_dir("dump");
    let first = run_in(&dir, &["--dump-default-config"], "");
    let second = run_in(&dir, &["--dump-default-config"], "");
    let written = dir.join("gdformatrc").is_file();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!((first.status, first.stderr.as_str()), (0, ""));
    assert!(written);
    assert_eq!(second.status, 1);
    assert_eq!(second.stderr, "AssertionError: gdformatrc already exists\n");
}

#[test]
fn an_unreadable_config_stops_the_run() {
    let dir = scratch_dir("config");
    std::fs::write(dir.join("a.gd"), "var x=1\n").unwrap();
    std::fs::write(dir.join("gdformatrc"), "line_length: [").unwrap();
    let ran = run_in(&dir, &["a.gd"], "");
    let untouched = std::fs::read_to_string(dir.join("a.gd")).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(ran.status, 1);
    assert!(
        ran.stderr.starts_with("gdformat: cannot read the config: "),
        "{}",
        ran.stderr
    );
    assert_eq!(untouched, "var x=1\n");
}

#[test]
fn check_names_the_files_that_would_change_and_leaves_them() {
    let dir = scratch_dir("check");
    std::fs::write(dir.join("a.gd"), "var x=1\n").unwrap();
    std::fs::write(dir.join("b.gd"), "var y = 2\n").unwrap();
    let ran = run_in(&dir, &["-c", "a.gd", "b.gd"], "");
    let untouched = std::fs::read_to_string(dir.join("a.gd")).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(ran.status, 1);
    assert_eq!(ran.stdout, "");
    assert_eq!(
        ran.stderr,
        "would reformat a.gd\n1 file would be reformatted, 1 file would be left unchanged.\n"
    );
    assert_eq!(untouched, "var x=1\n");
}

#[test]
fn formatting_in_place_names_the_files_it_rewrote() {
    let dir = scratch_dir("place");
    std::fs::write(dir.join("a.gd"), "var x=1\n").unwrap();
    std::fs::write(dir.join("b.gd"), "var y = 2\n").unwrap();
    let ran = run_in(&dir, &["a.gd", "b.gd"], "");
    let rewritten = std::fs::read_to_string(dir.join("a.gd")).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(ran.status, 0);
    assert_eq!(
        ran.stdout,
        "reformatted a.gd\n1 file reformatted, 1 file left unchanged.\n"
    );
    assert_eq!(rewritten, "var x = 1\n");
}

#[test]
fn a_lambda_header_keeps_its_name_arguments_and_return_type() {
    let code = "var a = func foo(x) -> int: return x\n\
                var b = func(x) -> int: return x\n\
                var c = func foo(x): return x\n\
                var d = func(): return 1\n\
                var e = func named() -> void: pass\n";
    assert_eq!(format_stdin(code), code);
}

#[test]
fn a_lone_abstract_stays_on_the_line_of_a_plain_func_but_not_a_static_one() {
    let formatted = format_stdin(
        "@abstract func a()\n@abstract static func c(): pass\n@abstract class_name Foo\n",
    );
    assert_eq!(
        formatted,
        "@abstract func a()\n@abstract\n\n\nstatic func c():\n\tpass\n\n\n@abstract class_name Foo\n"
    );
}

#[test]
fn annotations_join_a_variable_unless_a_function_follows_or_the_line_is_too_long() {
    let formatted = format_stdin(
        "@static_unload\n@export var x = 1\n\
         @export_range(0, 100) @onready var very_long_variable_name_here_to_exceed_the_line_length: int = 12345678901234567890\n\
         @warning_ignore(\"unused_variable\") var w\n\
         @rpc(\"any_peer\") func h(): pass\n",
    );
    assert_eq!(
        formatted,
        "@static_unload @export var x = 1\n\
         @export_range(0, 100)\n\
         @onready\n\
         var very_long_variable_name_here_to_exceed_the_line_length: int = 12345678901234567890\n\
         @warning_ignore(\"unused_variable\")\n\
         var w\n\
         @rpc(\"any_peer\")\n\
         func h():\n\tpass\n"
    );
}

#[test]
fn dot_chains_mixing_calls_and_subscripts_keep_their_order() {
    let code = "func f():\n\
                \ta.b(1).c[2].d(3)\n\
                \ta[1].b[2].c\n\
                \tx.y().z.w(1)[2].v\n\
                \ta.b[1][2].c(3)\n";
    assert_eq!(format_stdin(code), code);
}

#[test]
fn a_chain_too_long_for_the_line_breaks_before_each_dot() {
    let formatted = format_stdin(
        "var long_name = some_object.first_method(argument_one).second_attr[index_value].third_method(argument_two, argument_three).final_attr\n",
    );
    assert_eq!(
        formatted,
        "var long_name = (\n\
         \tsome_object\n\
         \t. first_method(argument_one)\n\
         \t. second_attr[index_value]\n\
         \t. third_method(argument_two, argument_three)\n\
         \t. final_attr\n\
         )\n"
    );
}

#[test]
fn standalone_and_inline_comments_inside_brackets_stay_where_they_were() {
    let formatted = format_stdin(
        "var a = [\n\t1, # one\n\t# standalone inside\n\t2,\n\t3 # three\n]\n\
         func f(\n\tx, # first\n\t# between\n\ty\n):\n\tpass\n",
    );
    assert_eq!(
        formatted,
        "var a = [\n\t1,  # one\n\t# standalone inside\n\t2,\n\t3  # three\n]\n\n\n\
         func f(\n\tx,  # first\n\t# between\n\ty\n):\n\tpass\n"
    );
}
