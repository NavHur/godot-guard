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


FOLDING_SOURCE = "var value = some_function(argument_one, argument_two, argument_three)\n"


class FormatterOracleTest(unittest.TestCase):
    def test_unfolded_follows_narrow_and_filtered_variants_are_skipped(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "a.gd"
            path.write_text(FOLDING_SOURCE, encoding="utf-8")
            with mock.patch.object(gdformat_cases, "VARIANTS", {"narrow", "unfolded"}):
                records = [json.loads(record) for record in gdformat_cases.cases(str(path))]
        self.assertEqual([record["variant"] for record in records], ["narrow", "unfolded"])
        narrow, unfolded = records
        self.assertGreater(narrow["expected"].count("\n"), 2)
        self.assertEqual(unfolded["code"], narrow["expected"])
        self.assertEqual(unfolded["expected"], FOLDING_SOURCE)

    def test_options_after_the_first_file_are_file_names(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "a.gd"
            path.write_text(FOLDING_SOURCE, encoding="utf-8")
            args = ["gdformat_cases.py", str(path), "--jobs", "9"]
            with (
                mock.patch.object(gdformat_cases.sys, "argv", args),
                contextlib.redirect_stdout(io.StringIO()),
                self.assertRaises(FileNotFoundError),
            ):
                gdformat_cases.main()

    def test_main_takes_options_before_the_files_and_formatted_trees_anywhere(self):
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "a.gd"
            path.write_text(FOLDING_SOURCE, encoding="utf-8")
            args = ["gdformat_cases.py", "--jobs", "1", "--variants", "narrow,unfolded", str(path), "--formatted-trees"]
            with mock.patch.object(gdformat_cases.sys, "argv", args), contextlib.redirect_stdout(output):
                gdformat_cases.main()
        records = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual([record["variant"] for record in records], ["narrow", "unfolded"])
        self.assertTrue(all(record["formatted"] is not None for record in records))

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
