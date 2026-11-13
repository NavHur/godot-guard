"""A temporary repository with fake gdformat, gdlint, and Godot on PATH, shared by the gate tests."""

from __future__ import annotations

import os
import stat
import subprocess
import unittest
from pathlib import Path
from unittest import mock

from .helpers import Repo

SHIM = """#!/bin/sh
echo "$(basename "$0") $*" >> "{log}"
{extra}
exit {status}
"""
CLEAN = "extends Node\n\nvar a := 1\n"
# Commits need an identity, and a CI runner has no git config to supply one.
GIT_IDENTITY = {
    "GIT_AUTHOR_NAME": "godot-guard tests",
    "GIT_AUTHOR_EMAIL": "tests@godot-guard.invalid",
    "GIT_COMMITTER_NAME": "godot-guard tests",
    "GIT_COMMITTER_EMAIL": "tests@godot-guard.invalid",
}


def git(root: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=root, check=True, capture_output=True, env={**os.environ, **GIT_IDENTITY})


class ShimTest(unittest.TestCase):
    """A temporary repository with fake gdformat, gdlint, and Godot on PATH."""

    def setUp(self):
        self.repo = Repo({"a.gd": CLEAN, "b.gd": CLEAN, "c.gd": CLEAN}, "paths: [.]\n")
        self.addCleanup(self.repo.cleanup)
        self.bin = self.repo.root / ".bin"
        self.log = self.repo.root / ".bin" / "calls.log"
        for name in ("gdformat", "gdlint", "godot"):
            self.shim(name)
        environment = {"PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}", "GODOT": str(self.bin / "godot")}
        patches = [
            mock.patch.dict(os.environ, environment),
            mock.patch("godot_guard.tools.sys.executable", "/nonexistent/python"),
        ]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)

    def shim(self, name: str, status: int = 0, extra: str = "") -> None:
        path = self.repo.write(f".bin/{name}", SHIM.format(log=self.log, extra=extra, status=status))
        path.chmod(path.stat().st_mode | stat.S_IEXEC)

    def calls(self) -> list[str]:
        return self.log.read_text().splitlines() if self.log.exists() else []
