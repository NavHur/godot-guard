"""The repository complexity check uses inclusive limits and measures every source."""

import contextlib
import io
import json
import unittest
from unittest import mock

from tools import check_complexity


def function(cognitive=6, cyclomatic=18):
    return {"name": "walk", "line": 3, "cognitive": cognitive, "cyclomatic": cyclomatic}


class ComplexityCheckTest(unittest.TestCase):
    def test_both_limits_are_inclusive(self):
        cases = [(6, 18, False), (7, 18, True), (6, 19, True), (0, 1, False), (None, 18, False), (None, 19, True)]
        for cognitive, cyclomatic, expected in cases:
            with self.subTest(cognitive=cognitive, cyclomatic=cyclomatic):
                self.assertEqual(check_complexity.is_over(function(cognitive, cyclomatic)), expected)

    def run_check(self, sources, reports):
        output = io.StringIO()
        patches = mock.patch.multiple(
            check_complexity,
            tracked_sources=mock.Mock(return_value=sources),
            measure=mock.Mock(side_effect=reports),
        )
        with patches, contextlib.redirect_stdout(output):
            status = check_complexity.check()
        return status, output.getvalue()

    def test_whole_tree_reports_violations_and_invalid_fixtures(self):
        fixture = "crates/gdscript/tests/fixtures/unexpected_eof.gd"
        reports = [
            {"a.py": [function()], "b.py": [function(7)]},
            {fixture: {"error": "UnexpectedToken: invalid fixture"}},
        ]
        sources = {"python": ["a.py", "b.py"], "rust": [], "gdscript": [fixture]}
        status, output = self.run_check(sources, reports)
        self.assertEqual(status, 1)
        self.assertEqual(
            output,
            (
                f"UNPARSED: {fixture}: UnexpectedToken: invalid fixture\n"
                "b.py:3: walk: cognitive 7, cyclomatic 18\n"
                "1 functions over cognitive 6 / cyclomatic 18 or sources not measured\n"
            ),
        )

    def test_a_clean_tree_passes(self):
        status, output = self.run_check({"rust": ["a.rs"]}, [{"a.rs": [function()]}])
        self.assertEqual(status, 0)
        self.assertEqual(output, "0 functions over cognitive 6 / cyclomatic 18 or sources not measured\n")

    def test_a_source_the_scorer_cannot_read_fails_the_check(self):
        status, output = self.run_check({"python": ["bom.py"]}, [{"bom.py": {"error": "SyntaxError: stray BOM"}}])
        self.assertEqual(status, 1)
        self.assertEqual(
            output,
            (
                "bom.py: not measured: SyntaxError: stray BOM\n"
                "1 functions over cognitive 6 / cyclomatic 18 or sources not measured\n"
            ),
        )

    def test_an_invalid_fixture_that_cannot_be_read_fails_the_check(self):
        fixture = "crates/gdscript/tests/fixtures/unexpected_eof.gd"
        error = "OSError: No such file or directory (os error 2)"
        status, output = self.run_check({"gdscript": [fixture]}, [{fixture: {"error": error}}])
        self.assertEqual(status, 1)
        self.assertEqual(
            output,
            (
                f"{fixture}: not measured: {error}\n"
                "1 functions over cognitive 6 / cyclomatic 18 or sources not measured\n"
            ),
        )

    def test_the_invalid_fixtures_are_tracked_gdscript_files(self):
        tracked = set(check_complexity.tracked_sources()["gdscript"])
        self.assertEqual(set(check_complexity.INVALID_FIXTURES) - tracked, set())

    def test_analyzer_cannot_silently_omit_a_file(self):
        result = mock.Mock(returncode=0, stdout=json.dumps({"a.py": []}))
        with mock.patch.object(check_complexity.subprocess, "run", return_value=result), \
                self.assertRaisesRegex(ValueError, r"missing \['b.py'\], unexpected \[\]"):
            check_complexity.measure("python", ["a.py", "b.py"])

    def test_analyzer_cannot_invent_a_file(self):
        result = mock.Mock(returncode=0, stdout=json.dumps({"a.py": [], "ghost.py": []}))
        with mock.patch.object(check_complexity.subprocess, "run", return_value=result), \
                self.assertRaisesRegex(ValueError, r"missing \[\], unexpected \['ghost.py'\]"):
            check_complexity.measure("python", ["a.py"])

    def test_a_failed_analyzer_is_not_a_clean_report(self):
        result = mock.Mock(returncode=2, stdout=json.dumps({"a.py": []}), stderr="fatal scorer failure\n")
        with mock.patch.object(check_complexity.subprocess, "run", return_value=result), \
                self.assertRaisesRegex(RuntimeError, "exited 2: fatal scorer failure"):
            check_complexity.measure("python", ["a.py"])

    def test_tracks_all_three_languages_without_path_exclusions(self):
        result = mock.Mock(stdout=b"src/a.py\0crates/b.rs\0fixtures/c.gd\0README.md\0")
        with mock.patch.object(check_complexity.subprocess, "run", return_value=result):
            self.assertEqual(
                check_complexity.tracked_sources(),
                {"python": ["src/a.py"], "rust": ["crates/b.rs"], "gdscript": ["fixtures/c.gd"]},
            )

    def test_tracked_filenames_keep_carriage_returns(self):
        result = mock.Mock(stdout=b"over\r.py\0")
        with mock.patch.object(check_complexity.subprocess, "run", return_value=result):
            self.assertEqual(check_complexity.tracked_sources()["python"], ["over\r.py"])

    def test_gdscript_files_named_like_options_are_paths(self):
        result = mock.Mock(returncode=0, stdout=json.dumps({"-x.gd": []}))
        with mock.patch.object(check_complexity.subprocess, "run", return_value=result) as run:
            check_complexity.measure("gdscript", ["-x.gd"])
        self.assertEqual(run.call_args.args[0][-2:], ["--", "-x.gd"])
