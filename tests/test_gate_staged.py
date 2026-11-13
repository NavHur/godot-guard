from __future__ import annotations

from godot_guard import config as config_module
from godot_guard import gate as gate_module

from .helpers import captured
from .shim import CLEAN, ShimTest, git

RESOURCE_BASE = "class_name Base\nextends Resource\n"
WIDE = "extends Base\n\n" + "".join(f"@export var v{n} := 1\n" for n in range(9))


class StagedContentTest(ShimTest):
    """The fast staged gate checks the index's content, not the working tree's."""

    def setUp(self):
        super().setUp()
        git(self.repo.root, "init", "-q")
        self.repo.write(".gdlintrc", "max-line-length: 100\n")
        self.shim("gdformat", extra=f'cat "$2" >> "{self.log}"')
        self.shim("gdlint", extra=f'cat .gdlintrc >> "{self.log}"')

    def stage(self, name: str, staged: str, working: str) -> None:
        self.repo.write(name, staged)
        git(self.repo.root, "add", name)
        self.repo.write(name, working)

    def test_invalid_staged_content_fails_with_a_clean_working_copy(self):
        self.stage("a.gd", "func f(:\n", CLEAN)
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 1)
        self.assertEqual(self.calls(), ["gdformat --check a.gd", "func f(:", "gdlint a.gd", "max-line-length: 100"])
        self.assertIn("a.gd:1: cannot parse", output)

    def test_valid_staged_content_passes_with_a_broken_working_copy(self):
        self.stage("a.gd", CLEAN, "func f(:\n")
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 0, output)

    def test_resource_exclusion_reads_the_index_for_selected_scripts(self):
        self.stage("base.gd", "class_name Base\nextends Node\n", "class_name Base\nextends Resource\n")
        self.stage("wide.gd", WIDE, WIDE)
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 1)
        self.assertIn("wide.gd:1: 9 inputs (@export 9) (max 8) (interface-cap)", output)

    def restage_base(self, delete_old: bool, add_new: bool) -> tuple[int, str]:
        """Commit z_old.gd declaring `Base extends Resource` and an `extends Base` script, restage, and gate."""
        self.stage("z_old.gd", RESOURCE_BASE, RESOURCE_BASE)
        self.stage("wide.gd", WIDE, WIDE)
        git(self.repo.root, "commit", "-qm", "base")
        if delete_old:
            git(self.repo.root, "rm", "-q", "--cached", "z_old.gd")
        if add_new:
            self.stage("a_new.gd", "class_name Base\nextends Node\n", "class_name Base\nextends Node\n")
        self.stage("wide.gd", WIDE + "\n", WIDE + "\n")
        return captured(gate_module.gate, self.repo.config(), True, True, [])

    def test_staged_class_name_beats_the_working_tree(self):
        status, output = self.restage_base(delete_old=False, add_new=True)
        self.assertEqual(status, 1, output)
        self.assertIn("wide.gd:1: 9 inputs", output)

    def test_scripts_staged_for_deletion_leave_the_resource_map(self):
        status, output = self.restage_base(delete_old=True, add_new=False)
        self.assertEqual(status, 1, output)
        self.assertIn("wide.gd:1: 9 inputs", output)

    def test_staged_deletion_and_new_declaration_of_a_class(self):
        status, output = self.restage_base(delete_old=True, add_new=True)
        self.assertEqual(status, 1, output)
        self.assertIn("wide.gd:1: 9 inputs", output)

    def test_resource_exclusion_reads_the_working_tree_for_unselected_scripts(self):
        source = self.repo.root / ".source"
        source.mkdir()
        (source / "base.gd").write_text("class_name Base\nextends Resource\n")
        git(source, "init", "-q")
        git(source, "add", "base.gd")
        git(source, "commit", "-qm", "base")
        git(self.repo.root, "-c", "protocol.file.allow=always", "submodule", "add", "-q", str(source), "vendor")
        self.stage("wide.gd", WIDE, WIDE)
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 0, output)
        self.assertIn("0 errors, 0 warnings, 1 file", output)

    def test_unchanged_symlink_to_a_non_script_is_ignored(self):
        self.repo.write("target.txt", CLEAN)
        (self.repo.root / "alias.gd").symlink_to("target.txt")
        git(self.repo.root, "add", "target.txt", "alias.gd")
        git(self.repo.root, "commit", "-qm", "alias")
        self.stage("a.gd", CLEAN, CLEAN)
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 0, output)
        self.assertIn("0 errors, 0 warnings, 1 file", output)

    def test_staged_symlinked_scripts_fail_the_gate(self):
        self.stage("target.gd", CLEAN, CLEAN)
        self.stage("retyped.gd", CLEAN, CLEAN)
        git(self.repo.root, "commit", "-qm", "scripts")
        (self.repo.root / "retyped.gd").unlink()
        for name in ("alias.gd", "retyped.gd"):
            (self.repo.root / name).symlink_to("target.gd")
        git(self.repo.root, "add", "alias.gd", "retyped.gd")
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 1, output)
        message = "{}: symlinked scripts are not supported; commit the real file"
        self.assertEqual(output.splitlines(), [message.format("alias.gd"), message.format("retyped.gd")])
        self.assertEqual(self.calls(), [])

    def test_baseline_comes_from_the_index(self):
        self.stage("wide.gd", WIDE.replace("Base", "Node"), WIDE.replace("Base", "Node"))
        self.stage(".godot-guard-baseline.json", '{"interface-cap": {"wide.gd": 9}}\n', '{"interface-cap": {}}\n')
        status, output = captured(gate_module.gate, self.repo.config(), True, True, [])
        self.assertEqual(status, 0, output)

    def test_config_root_below_the_git_root(self):
        game = self.repo.root / "game"
        self.repo.write("game/project.godot", "config_version=5\n")
        self.repo.write("game/.godot-guard.yml", "paths: [.]\n")
        self.stage("game/scripts/a.gd", "func f(:\n", CLEAN)
        self.stage("other.gd", "func f(:\n", CLEAN)
        status, output = captured(gate_module.gate, config_module.load(game), True, True, [])
        self.assertEqual(status, 1)
        self.assertEqual(self.calls()[0], "gdformat --check scripts/a.gd")
        self.assertIn("scripts/a.gd:1: cannot parse", output)
        self.assertIn("1 file", output)
