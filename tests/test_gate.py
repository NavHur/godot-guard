from __future__ import annotations

import contextlib
import io
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from godot_guard import gate as gate_module
from godot_guard.cli import format_files, main
from godot_guard.tools import ToolError, godot, run_godot

from .helpers import Repo, captured
from .shim import CLEAN, ShimTest, git


class FastGateTest(ShimTest):
    def setUp(self):
        super().setUp()
        git(self.repo.root, "init", "-q")
        git(self.repo.root, "add", "a.gd", "b.gd")
        (self.repo.root / "b.gd").unlink()

    def test_staged_files_are_checked_even_when_deleted_from_the_working_tree(self):
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 0, output)
        self.assertEqual(self.calls(), ["gdformat --check a.gd b.gd", "gdlint a.gd b.gd"])
        self.assertIn("0 errors, 0 warnings, 2 files", output)
        self.assertIn("== check passed in", output)

    def test_nothing_staged_is_nothing_to_do(self):
        git(self.repo.root, "rm", "-q", "--cached", "a.gd", "b.gd")
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 0)
        self.assertIn("no GDScript files selected", output)
        self.assertEqual(self.calls(), [])

    def test_failing_format_stops_the_gate(self):
        self.shim("gdformat", status=1)
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 1)
        self.assertIn("== format failed", output)
        self.assertEqual(self.calls(), ["gdformat --check a.gd b.gd"])


class FullGateTest(ShimTest):
    def test_runs_every_step_in_order(self):
        self.repo.write(
            ".godot-guard.yml",
            (
                "paths: [scripts, .]\nexclude: [addons/**]\n"
                "gate:\n  steps: ['echo step >> .bin/calls.log']\n  tests: [res://tests/run_tests.gd]\n"
            ),
        )
        status, output = captured(gate_module.gate, self.repo.config(), False, False, [])
        self.assertEqual(status, 0, output)
        analyzer = str(gate_module.ANALYZER)
        root = str(self.repo.root)
        self.assertEqual(
            self.calls(),
            [
                "gdformat --check a.gd b.gd c.gd",
                "gdlint a.gd b.gd c.gd",
                "step",
                f"godot --headless --path {root} --editor --quit",
                f"godot --headless --path {root} --script {analyzer} -- --root res://scripts --root res:// --exclude addons/**",
                f"godot --headless --path {root} --script res://tests/run_tests.gd",
                f"godot --headless --path {root} --quit-after 3",
            ],
        )
        self.assertTrue(gate_module.ANALYZER.is_file())

    def test_godot_warning_fails_the_gate(self):
        self.shim("godot", extra="echo 'WARNING: something odd'")
        status, output = captured(gate_module.gate, self.repo.config(), False, False, [])
        self.assertEqual(status, 1)
        self.assertIn("== import failed", output)


class ShardedTestsTest(ShimTest):
    def test_sharded_entries_run_one_process_per_shard(self):
        self.repo.write(
            ".godot-guard.yml",
            (
                "gate:\n  format: false\n  lint: false\n  import: false\n  analyzer: false\n  smoke: false\n"
                "  tests: [{script: res://tests/run_tests.gd, shards: 2}]\n"
            ),
        )
        status, output = captured(gate_module.gate, self.repo.config(), False, False, [])
        self.assertEqual(status, 0, output)
        root = str(self.repo.root)
        self.assertEqual(
            self.calls(),
            [
                f"godot --headless --path {root} --script res://tests/run_tests.gd -- --shard=1/2",
                f"godot --headless --path {root} --script res://tests/run_tests.gd -- --shard=2/2",
            ],
        )
        self.assertIn("== tests res://tests/run_tests.gd [2/2]", output)

    def test_a_mapping_with_one_shard_passes_the_argument_and_a_string_does_not(self):
        self.repo.write(
            ".godot-guard.yml",
            (
                "gate:\n  format: false\n  lint: false\n  import: false\n  analyzer: false\n  smoke: false\n"
                "  tests: [res://tests/plain.gd, {script: res://tests/one.gd, shards: 1}]\n"
            ),
        )
        status, output = captured(gate_module.gate, self.repo.config(), False, False, [])
        self.assertEqual(status, 0, output)
        root = str(self.repo.root)
        self.assertEqual(
            self.calls(),
            [
                f"godot --headless --path {root} --script res://tests/plain.gd",
                f"godot --headless --path {root} --script res://tests/one.gd -- --shard=1/1",
            ],
        )
        self.assertIn("== tests res://tests/one.gd [1/1]", output)


class ParserWarmupTest(ShimTest):
    def test_only_a_parallel_gate_warms_the_parser_cache(self):
        for jobs, warmed in ((1, False), (2, True)):
            self.repo.write(".godot-guard.yml", f"gate: {{jobs: {jobs}}}\n")
            with self.subTest(jobs=jobs), mock.patch.object(gate_module, "warm_parser_cache") as warm:
                captured(gate_module.gate, self.repo.config(), True, False, [])
                self.assertEqual(warm.called, warmed)


class ParserCacheTest(unittest.TestCase):
    def test_a_cold_cache_gets_its_directory_and_every_pickle(self):
        # A fresh HOME is a cold cache. The warm-up runs in a child, since ShimTest patches sys.executable.
        with tempfile.TemporaryDirectory() as home:
            code = "from godot_guard.gate import warm_parser_cache; warm_parser_cache()"
            subprocess.run(
                [sys.executable, "-c", code],
                check=True,
                timeout=120,
                env=dict(os.environ, HOME=home, PYTHONPATH=str(Path(gate_module.__file__).parents[1])),
            )
            pickles = sorted(path.name for path in Path(home).rglob("*.pickle"))
        self.assertEqual(pickles, ["parser.pickle", "parser_comments.pickle", "parser_with_metadata.pickle"])


class DirectoryArgumentTest(ShimTest):
    def setUp(self):
        super().setUp()
        for name in ("scripts/d.gd", "scripts/gen/e.gd", "scripts/.hidden/f.gd", "scripts/notes.txt"):
            self.repo.write(name, CLEAN)
        self.repo.write(".godot-guard.yml", "paths: [.]\nexclude: [scripts/gen]\n")

    def test_format_expands_directories(self):
        status, output = captured(
            format_files, self.repo.config(), [str(self.repo.root / "scripts"), str(self.repo.root / "a.gd")]
        )
        self.assertEqual(status, 0, output)
        self.assertEqual(self.calls(), ["gdformat a.gd scripts/d.gd"])

    def test_fast_gate_expands_directories(self):
        status, output = captured(gate_module.gate, self.repo.config(), True, False, [str(self.repo.root / "scripts")])
        self.assertEqual(status, 0, output)
        self.assertEqual(self.calls(), ["gdformat --check scripts/d.gd", "gdlint scripts/d.gd"])

    def test_hidden_or_excluded_arguments_select_nothing(self):
        names = ["scripts/.hidden", "scripts/.hidden/f.gd", "scripts/gen", "scripts/gen/e.gd"]
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            status, output = captured(format_files, self.repo.config(), [str(self.repo.root / n) for n in names])
        self.assertEqual(status, 0, output)
        self.assertIn("no GDScript files selected", output)
        self.assertEqual(self.calls(), [])
        for name in names:
            self.assertIn(f"skipping {name}:", errors.getvalue())
        self.assertIn("scripts/.hidden: hidden directories are not scanned", errors.getvalue())
        self.assertIn("scripts/gen: it matches exclude", errors.getvalue())

    def test_links_in_hidden_or_excluded_directories_select_nothing(self):
        links = {".hidden/alias.gd": "../a.gd", ".hidden/directory": "../scripts", "scripts/gen/alias.gd": "../d.gd"}
        for name, target in links.items():
            (self.repo.root / name).parent.mkdir(exist_ok=True)
            (self.repo.root / name).symlink_to(target)
        names = [".hidden/alias.gd", ".hidden/directory", ".hidden/directory/d.gd", "scripts/gen/alias.gd"]
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            status, output = captured(format_files, self.repo.config(), [str(self.repo.root / n) for n in names])
        self.assertEqual(status, 0, output)
        self.assertEqual(self.calls(), [])
        for name in names:
            self.assertIn(f"skipping {name}:", errors.getvalue())

    def test_staged_scripts_in_hidden_directories_are_not_selected(self):
        git(self.repo.root, "init", "-q")
        git(self.repo.root, "add", "scripts/.hidden/f.gd")
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 0, output)
        self.assertIn("no GDScript files selected", output)


class RunTest(ShimTest):
    def test_warning_with_exit_zero_fails(self):
        self.shim("godot", extra="echo 'WARNING: unused variable'")
        self.assertEqual(captured(run_godot, ["--version"])[0], 1)

    def test_script_error_anywhere_in_a_line_fails(self):
        self.shim("godot", extra="echo '  SCRIPT ERROR: Parse Error'")
        self.assertEqual(captured(run_godot, [])[0], 1)

    def test_clean_output_passes_and_exit_status_is_kept(self):
        self.shim("godot", extra="echo 'Godot Engine v4.7.1'")
        self.assertEqual(captured(main, ["run", "--", "--headless", "--version"])[0], 0)
        self.assertEqual(self.calls(), ["godot --headless --version"])
        self.shim("godot", status=3)
        self.assertEqual(captured(run_godot, [])[0], 3)

    def test_captured_output_is_echoed_and_warnings_still_fail(self):
        self.shim("godot", extra="echo 'WARNING: unused variable'")
        lines: list[str] = []
        self.assertEqual(run_godot([], echo=lines.append), 1)
        self.assertEqual(lines, ["WARNING: unused variable\n"])

    def test_captured_clean_output_passes_and_exit_status_is_kept(self):
        self.shim("godot", extra="echo 'Godot Engine v4.7.1'")
        lines: list[str] = []
        self.assertEqual(run_godot(["--version"], echo=lines.append), 0)
        self.assertEqual(lines, ["Godot Engine v4.7.1\n"])
        self.shim("godot", status=3, extra="echo 'bye'")
        self.assertEqual(run_godot([], echo=lines.append), 3)
        self.assertEqual(lines[-1], "bye\n")


def real_godot() -> str | None:
    try:
        return godot()
    except ToolError:
        return None


@unittest.skipUnless(real_godot(), "Godot 4 not found")
class AnalyzerTest(unittest.TestCase):
    """The packaged analyzer script under a real headless Godot."""

    def setUp(self):
        self.repo = Repo({"a.gd": CLEAN, "scripts/b.gd": CLEAN, "empty/notes.txt": ""})
        self.addCleanup(self.repo.cleanup)

    def analyze(self, *roots: str) -> tuple[int, str]:
        args = [arg for root in roots for arg in ("--root", root)]
        return captured(
            run_godot, ["--headless", "--path", str(self.repo.root), "--script", str(gate_module.ANALYZER), "--", *args]
        )

    def test_file_roots_are_analyzed(self):
        status, output = self.analyze("res://a.gd")
        self.assertEqual(status, 0, output)
        self.assertIn("Analyzed 1 scripts, 0 failed to load.", output)

    def test_missing_root_fails(self):
        status, output = self.analyze("res://missing", "res://a.gd")
        self.assertEqual(status, 1, output)
        self.assertIn("ANALYZER: root res://missing does not exist", output)

    def test_roots_without_scripts_fail(self):
        status, output = self.analyze("res://empty")
        self.assertEqual(status, 1, output)
        self.assertIn("ANALYZER: no scripts under res://empty", output)


if __name__ == "__main__":
    unittest.main()
