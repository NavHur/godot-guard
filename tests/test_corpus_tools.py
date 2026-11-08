"""Stable mutation sequences and formatter-oracle record ordering."""

import contextlib
import io
import json
import random
import tempfile
import unittest
from pathlib import Path
from unittest import mock
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

    def test_main_keeps_option_order_and_prints_records_in_pool_order(self):
        output = io.StringIO()
        args = ["gdformat_cases.py", "--formatted-trees", "--variants", "narrow,unfolded", "--jobs", "2", "a.gd"]
        with (
            mock.patch.object(gdformat_cases.sys, "argv", args),
            mock.patch.object(gdformat_cases.multiprocessing, "Pool") as pool,
            contextlib.redirect_stdout(output),
        ):
            pool.return_value.__enter__.return_value.imap.return_value = [["one", "two"], ["three"]]
            gdformat_cases.main()
        self.assertEqual(output.getvalue(), "one\ntwo\nthree\n")
        pool.assert_called_once_with(2, initializer=gdformat_cases._init, initargs=(True, {"narrow", "unfolded"}))
        pool.return_value.__enter__.return_value.imap.assert_called_once_with(
            gdformat_cases.cases, ["a.gd"], chunksize=4
        )
