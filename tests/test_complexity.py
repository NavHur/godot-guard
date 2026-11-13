from __future__ import annotations

import unittest

from gdtoolkit.parser import parser

from godot_guard import complexity

from . import fixtures


def tree(source: str):
    return parser.parse(source, gather_metadata=True)


class ComplexityTest(unittest.TestCase):
    def test_scores_every_function(self):
        scores = {f.name: (f.line, f.cognitive, f.cyclomatic) for f in complexity.score(tree(fixtures.COMPLEXITY))}
        self.assertEqual(scores, fixtures.COMPLEXITY_SCORES)

    def test_lambda_folds_into_enclosing_cognitive(self):
        source = "func f(x):\n\tvar g = func(y):\n\t\tif y:\n\t\t\treturn 1\n\t\treturn 0\n\treturn g\n"
        functions = {f.name: f for f in complexity.score(tree(source))}
        self.assertEqual(functions["f"].cognitive, 2)
        self.assertEqual(functions["f.<lambda#1>"].cyclomatic, 2)

    def test_lambdas_are_named_after_their_enclosing_function(self):
        source = (
            "var top = func(): return 1\n\n\nfunc f():\n\tvar a = func(): return func(): return 2\n"
            "\tvar b = func named(): return 3\n\tvar c = func(): return 4\n\n\nclass Inner:\n\tvar d = func(): return 5\n"
        )
        names = [f.name for f in complexity.score(tree(source))]
        self.assertEqual(
            names,
            [
                "<lambda#1>",
                "f",
                "f.<lambda#1>",
                "f.<lambda#1>.<lambda#1>",
                "f.named",
                "f.<lambda#2>",
                "Inner.<lambda#1>",
            ],
        )

    def test_nested_ternary_reports_outermost_once(self):
        self.assertEqual(complexity.nested_ternaries(tree(fixtures.NESTED_TERNARY)), [6, 7, 8])

    def test_flat_ternaries_are_not_nested(self):
        source = "func f(a, b):\n\tvar x = 1 if a else 2\n\tvar y = 3 if b else 4\n"
        self.assertEqual(complexity.nested_ternaries(tree(source)), [])

    def test_symbolic_boolean_operators_score_like_keyword_operators(self):
        for words, symbols in [
            ("a and b and c", "a && b && c"),
            ("a or b or c", "a || b || c"),
            ("a and b or c", "a && b || c"),
            ("a and b or c and d", "a && b || c && d"),
        ]:
            with self.subTest(condition=symbols):
                self.assertEqual(self.scores(words), self.scores(symbols))
        self.assertEqual(self.scores("a && b && c"), (1, 3))
        self.assertEqual(self.scores("a && b || c"), (2, 3))
        self.assertEqual(self.scores("a && b || c && d"), (3, 4))

    def scores(self, condition: str) -> tuple[int | None, int]:
        source = f"func f(a, b, c, d):\n\tvar x = {condition}\n"
        (function,) = complexity.score(tree(source))
        return function.cognitive, function.cyclomatic


if __name__ == "__main__":
    unittest.main()
