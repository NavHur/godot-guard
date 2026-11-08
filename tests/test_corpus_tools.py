"""Stable mutation sequences and formatter-oracle record ordering."""

import contextlib
import io
import json
import random
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools.gdscript import mutate_corpus
from tools.gdscript.mutate_corpus import mutate
from tools.oracle import gdformat_cases


class MutationTest(unittest.TestCase):
    def test_seeded_edits_keep_the_original_random_sequence(self):
        expected = [
            "alpha\nbeta\nga",
            "al,pha\nbeta\ngamma",
            "al",
            "alph^a\nbeta\ngamma",
            'alpha\nbet"a\ngamma',
            "alpha\nbeta\nbeta\ngamma",
            "al",
            " alpha\nbeta\ngamma",
            "alpha\nbeta\n&gamma",
            "alpha\nbeta\namma",
            "alpha\nalpha\nbeta\ngamma",
            "alpha\nbeta\namma",
            "alpha\neta\ngamma",
            "alpha\n  beta\ngamma",
            "alpha\nbta\ngamma",
            "1ealpha\nbeta\ngamma",
        ]
        actual = [mutate("alpha\nbeta\ngamma", random.Random(seed)) for seed in range(16)]
        self.assertEqual(actual, expected)

    def test_line_deletion_keeps_the_original_random_sequence(self):
        actual = [mutate("alpha\nbeta\ngamma", random.Random(seed)) for seed in (19, 20, 24)]
        self.assertEqual(actual, ["beta\ngamma", "alpha\nbeta", "alpha\ngamma"])

    def test_one_generator_across_files_keeps_the_original_random_sequence(self):
        sources = ["alpha\nbeta", "gamma", "one\ntwo\nthree"]
        with tempfile.TemporaryDirectory() as root:
            paths = []
            for index, source in enumerate(sources):
                path = Path(root) / f"source{index}.gd"
                path.write_text(source, encoding="utf-8")
                paths.append(str(path))
            out = Path(root) / "out"
            with mock.patch.object(mutate_corpus.sys, "argv", ["mutate_corpus.py", str(out), "10", "5", *paths]):
                mutate_corpus.main()
            actual = [path.read_text(encoding="utf-8") for path in sorted(out.glob("mutant_*.gd"))]
        self.assertEqual(
            actual,
            [
                "one\ntwo\n three",
                "one\ntwo\nth",
                "one\ntwothree",
                "beta",
                "alphabeta",
                "gam",
                "one\ntwo\ntree",
                "alpa\nbeta",
                " gamma",
                "al",
            ],
        )

    def test_empty_input_keeps_the_same_insertions(self):
        actual = [mutate("", random.Random(seed)) for seed in range(8)]
        self.assertEqual(actual, ["", ",", "", "^", '"', "\n", "", " "])


class FormatterOracleTest(unittest.TestCase):
    def test_unfolded_follows_narrow_and_filtered_variants_are_skipped(self):
        variants = [
            ("original", "code", 100, None),
            ("invalid", None, 100, None),
            ("narrow", "narrow code", 40, None),
            ("spaces4", "spaces code", 100, 4),
        ]
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "a.gd"
            path.write_text("code", encoding="utf-8")
            with (
                mock.patch.object(gdformat_cases, "VARIANTS", {"narrow", "unfolded"}),
                mock.patch.object(gdformat_cases, "variants", return_value=variants),
                mock.patch.object(
                    gdformat_cases, "case", side_effect=[{"expected": "folded"}, {"expected": "wide"}]
                ) as case,
            ):
                records = gdformat_cases.cases(str(path))
        self.assertEqual([json.loads(record) for record in records], [{"expected": "folded"}, {"expected": "wide"}])
        self.assertEqual(
            case.call_args_list,
            [
                mock.call(str(path), "narrow", "narrow code", 40, None),
                mock.call(str(path), "unfolded", "folded", 100, None),
            ],
        )

    def test_options_accept_flags_in_any_order_before_the_files(self):
        args = ["--variants", "narrow,unfolded", "--formatted-trees", "--jobs", "2", "a.gd", "--jobs", "9"]
        self.assertEqual(gdformat_cases.options(args), (True, 2, {"narrow", "unfolded"}, ["a.gd", "--jobs", "9"]))
        self.assertEqual(gdformat_cases.options(["a.gd"]), (False, 4, None, ["a.gd"]))

    def test_main_prints_each_files_records_in_file_order_with_formatted_trees(self):
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as root:
            paths = []
            for name in ("a.gd", "b.gd", "c.gd"):
                path = Path(root) / name
                path.write_text("extends Node\n\nvar value=1\n", encoding="utf-8")
                paths.append(str(path))
            args = ["gdformat_cases.py", "--formatted-trees", "--variants", "original", "--jobs", "2", *paths]
            with mock.patch.object(gdformat_cases.sys, "argv", args), contextlib.redirect_stdout(output):
                gdformat_cases.main()
        records = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual([record["id"] for record in records], [f"{path}#original" for path in paths])
        for record in records:
            self.assertEqual((record["expected"], record["safety"]), ("extends Node\n\nvar value = 1\n", "ok"))
            self.assertEqual(sorted(record["formatted"]), ["comments", "tree"])
            self.assertTrue(record["formatted"]["tree"] and record["formatted"]["comments"])
