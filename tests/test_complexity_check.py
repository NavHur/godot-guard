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

    def test_whole_tree_reports_violations_and_invalid_fixtures(self):
        output = io.StringIO()
        reports = [
            {"a.py": [function()], "b.py": [function(7)]},
            {"bad.gd": {"error": "invalid fixture"}},
        ]
        sources = {"python": ["a.py", "b.py"], "rust": [], "gdscript": ["bad.gd"]}
        with (
            mock.patch.object(check_complexity, "tracked_sources", return_value=sources),
            mock.patch.object(check_complexity, "measure", side_effect=reports) as measure,
            contextlib.redirect_stdout(output),
        ):
            status = check_complexity.check()
        self.assertEqual(status, 1)
        self.assertEqual(
            output.getvalue(),
            (
                "UNPARSED: bad.gd: invalid fixture\n"
                "b.py:3: walk: cognitive 7, cyclomatic 18\n"
                "1 functions over cognitive 6 / cyclomatic 18\n"
            ),
        )
        self.assertEqual(
            measure.call_args_list, [mock.call("python", ["a.py", "b.py"]), mock.call("gdscript", ["bad.gd"])]
        )

    def test_a_clean_tree_passes(self):
        with (
            mock.patch.object(check_complexity, "tracked_sources", return_value={"rust": ["a.rs"]}),
            mock.patch.object(check_complexity, "measure", return_value={"a.rs": [function()]}),
            contextlib.redirect_stdout(io.StringIO()) as output,
        ):
            status = check_complexity.check()
        self.assertEqual(status, 0)
        self.assertEqual(output.getvalue(), "0 functions over cognitive 6 / cyclomatic 18\n")

    def test_analyzer_cannot_silently_omit_a_file(self):
        result = mock.Mock(stdout=json.dumps({"a.py": []}))
        with mock.patch.object(check_complexity.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(ValueError, "exactly the requested files"):
                check_complexity.measure("python", ["a.py", "b.py"])

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
        result = mock.Mock(stdout=json.dumps({"-x.gd": []}))
        with mock.patch.object(check_complexity.subprocess, "run", return_value=result) as run:
            check_complexity.measure("gdscript", ["-x.gd"])
        self.assertEqual(run.call_args.args[0][-2:], ["--", "-x.gd"])
