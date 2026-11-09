"""Check every tracked Python, Rust, and GDScript function against this repo's limits.

Run with pbp-complexity v0.2.0 and godot-guard v0.7.0 on PATH, or set
PBP_COMPLEXITY and PBP_GODOT_GUARD to their executable paths.
"""

import json
import os
import subprocess
from pathlib import Path

LIMITS = {"cognitive": 6, "cyclomatic": 18}
LIMITS_TEXT = " / ".join(f"{metric} {limit}" for metric, limit in LIMITS.items())
LANGUAGES = {".py": "python", ".rs": "rust", ".gd": "gdscript"}
# Parser fixtures that are invalid on purpose, with the parse error each must report.
# Any other error, or any other file the scorer cannot measure, is a failure.
INVALID_FIXTURES = {
    "crates/gdformat/tests/fixtures/cli/broken/bad.gd": "UnexpectedToken",
    "crates/gdscript/tests/fixtures/dedent_error.gd": "DedentError",
    "crates/gdscript/tests/fixtures/eof_dedent_in_brackets.gd": "UnexpectedToken",
    "crates/gdscript/tests/fixtures/unexpected_character.gd": "UnexpectedCharacters",
    "crates/gdscript/tests/fixtures/unexpected_eof.gd": "UnexpectedToken",
}
ROOT = Path(__file__).resolve().parents[1]


def tracked_sources():
    # Bytes, not text: universal-newline decoding would turn a "\r" in a filename into "\n".
    result = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True)
    groups = {language: [] for language in LANGUAGES.values()}
    for raw_path in result.stdout.split(b"\0"):
        path = os.fsdecode(raw_path)
        language = LANGUAGES.get(Path(path).suffix)
        if language:
            groups[language].append(path)
    return groups


def analyzer(language):
    if language == "gdscript":
        # "--" so a root-level file named like "-x.gd" is a path, not an option.
        return [os.environ.get("PBP_GODOT_GUARD", "godot-guard"), "complexity", "--json", "--"]
    return [os.environ.get("PBP_COMPLEXITY", "pbp-complexity"), language]


def measure(language, paths):
    result = subprocess.run([*analyzer(language), *paths], check=False, cwd=ROOT, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"{language}: analyzer exited {result.returncode}: {result.stderr.strip()}")
    report = json.loads(result.stdout)
    missing, extra = sorted(set(paths) - set(report)), sorted(set(report) - set(paths))
    if missing or extra:
        raise ValueError(
            f"{language}: analyzer did not report exactly the requested files: missing {missing}, unexpected {extra}"
        )
    return report


def is_over(function):
    return any(function[metric] is not None and function[metric] > limit for metric, limit in LIMITS.items())


def describe(path, function):
    scores = ", ".join(f"{metric} {function[metric]}" for metric in LIMITS)
    return f"{path}:{function['line']}: {function['name']}: {scores}"


def is_expected_parse_error(path, error):
    expected = INVALID_FIXTURES.get(path)
    return expected is not None and error.startswith(f"{expected}:")


def file_findings(path, functions):
    if isinstance(functions, dict):
        if not is_expected_parse_error(path, functions["error"]):
            return [f"{path}: not measured: {functions['error']}"]
        print(f"UNPARSED: {path}: {functions['error']}")
        return []
    return [describe(path, function) for function in functions if is_over(function)]


def check():
    findings = []
    for language, paths in tracked_sources().items():
        if not paths:
            continue
        for path, functions in measure(language, paths).items():
            findings.extend(file_findings(path, functions))
    for finding in findings:
        print(finding)
    print(f"{len(findings)} functions over {LIMITS_TEXT} or sources not measured")
    return int(bool(findings))


if __name__ == "__main__":
    raise SystemExit(check())
