"""Interface extraction across annotation groups and inheritance chains."""

import unittest

from godot_guard.interface import Surface, is_resource, parse


class InterfaceEdgesTest(unittest.TestCase):
    def test_annotation_groups_end_at_a_declaration(self):
        source = """extends Node
@export
@export_range(0, 10)
var first: int
var plain: int
@export_category("Settings")
var category_only: int
@export
signal changed
var after_signal: int
@export var inline: int
"""
        self.assertEqual(parse("a.gd", source).exports, ["first", "inline"])

    def test_resource_chain_can_follow_paths_and_class_names(self):
        resource = Surface("base.gd", extends="Resource", class_name="Base")
        middle = Surface("middle.gd", extends="Base")
        surface = Surface("leaf.gd", extends="res://middle.gd")
        self.assertTrue(is_resource(surface, {"Base": resource}, {"middle.gd": middle}))

    def test_cycles_and_unknown_bases_are_not_resources(self):
        left = Surface("left.gd", extends="Right", class_name="Left")
        right = Surface("right.gd", extends="Left", class_name="Right")
        self.assertFalse(is_resource(left, {"Left": left, "Right": right}, {}))
        self.assertFalse(is_resource(Surface("unknown.gd", extends="Missing"), {}, {}))
