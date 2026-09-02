"""Tests for scripts/check_architecture.py."""

from __future__ import annotations

import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS_DIR))

import check_architecture  # noqa: E402

LAYERS = """
[layer.foundation]
paths = ["src/value/**", "src/foundation/**"]
may_depend_on = []

[layer.action]
paths = ["src/action/**"]
may_depend_on = ["foundation"]

[layer.runtime]
paths = ["src/runtime/**"]
may_depend_on = ["foundation", "action"]

[layer.introspection]
paths = ["src/introspection/**"]
may_depend_on = ["foundation", "action"]

[layer.root]
paths = ["src/lib.rs"]
may_depend_on = ["foundation", "action", "runtime", "introspection"]
"""


def write_tree(root: Path, files: dict[str, str]) -> None:
    for relative, content in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(textwrap.dedent(content).lstrip("\n"), encoding="utf-8")


def run_in(root: Path, extra: list[str] | None = None) -> tuple[int, str]:
    argv = ["--root", str(root), "--config", str(root / "architecture.toml")]
    if extra:
        argv.extend(extra)
    return check_architecture.run(argv)


class CheckArchitectureTests(unittest.TestCase):
    def test_pass_when_dependencies_follow_layers(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/lib.rs": """
                        mod foundation;
                        mod action;
                    """,
                    "src/foundation/mod.rs": """
                        pub struct Value;
                    """,
                    "src/action/mod.rs": """
                        use crate::foundation::Value;
                        pub struct Action(pub Value);
                    """,
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 0, output)
        self.assertIn("Architecture check passed", output)
        self.assertIn("Files checked:", output)
        self.assertIn("Dependency edges:", output)
        self.assertIn("Violations: 0", output)

    def test_fail_when_lower_layer_depends_on_higher_layer(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/value/foo.rs": """
                        use crate::runtime::Runtime;
                    """,
                    "src/runtime/mod.rs": """
                        pub struct Runtime;
                    """,
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 1, output)
        self.assertIn("Architecture check failed:", output)
        self.assertIn("src/value/foo.rs", output.replace("\\", "/"))
        self.assertIn("forbidden dependency:", output)
        self.assertIn("foundation -> runtime", output)
        self.assertIn("use crate::runtime::Runtime;", output)

    def test_fail_on_module_cycle(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/foundation/a.rs": """
                        use crate::foundation::b::B;
                        pub struct A;
                    """,
                    "src/foundation/b.rs": """
                        use crate::foundation::a::A;
                        pub struct B;
                    """,
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 1, output)
        self.assertIn("circular dependency:", output)
        self.assertIn("foundation::a", output)
        self.assertIn("foundation::b", output)

    def test_fail_when_production_code_depends_on_tests(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/foundation/mod.rs": """
                        use crate::tests::fixture::Fixture;
                    """,
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 1, output)
        self.assertIn("production -> tests", output)

    def test_fail_when_bypassing_internal_module(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = LAYERS.replace(
                'may_depend_on = ["foundation"]',
                'may_depend_on = ["foundation"]\ninternal_modules = ["action::internal"]',
                1,
            )
            write_tree(
                root,
                {
                    "architecture.toml": config,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/action/mod.rs": "pub struct Action;\n",
                    "src/action/internal.rs": "pub struct Secret;\n",
                    "src/runtime/mod.rs": """
                        use crate::action::internal::Secret;
                    """,
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 1, output)
        self.assertIn("internal module", output.lower())
        self.assertIn("action::internal", output)

    def test_allow_rule_permits_otherwise_forbidden_edge(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = LAYERS + textwrap.dedent(
                """
                [[allow]]
                from = "src/foundation/special.rs"
                to = "runtime"
                """
            )
            write_tree(
                root,
                {
                    "architecture.toml": config,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/foundation/special.rs": """
                        use crate::runtime::Runtime;
                    """,
                    "src/runtime/mod.rs": "pub struct Runtime;\n",
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 0, output)
        self.assertIn("Architecture check passed", output)

    def test_deny_rule_blocks_otherwise_allowed_edge(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = LAYERS + textwrap.dedent(
                """
                [[deny]]
                from = "runtime"
                to = "action"
                """
            )
            write_tree(
                root,
                {
                    "architecture.toml": config,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/action/mod.rs": "pub struct Action;\n",
                    "src/runtime/mod.rs": """
                        use crate::action::Action;
                    """,
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 1, output)
        self.assertIn("forbidden dependency:", output)
        self.assertIn("runtime -> action", output)

    def test_comments_are_not_treated_as_imports(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/foundation/mod.rs": """
                        // use crate::runtime::Runtime;
                        /* use crate::action::Action; */
                        pub struct Value;
                    """,
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 0, output)

    def test_cfg_test_imports_are_ignored(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/foundation/mod.rs": """
                        pub struct Value;

                        #[cfg(test)]
                        mod tests {
                            use crate::runtime::Runtime;
                        }
                    """,
                    "src/runtime/mod.rs": "pub struct Runtime;\n",
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 0, output)

    def test_super_import_same_layer_is_allowed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/foundation/mod.rs": "pub struct Parent;\n",
                    "src/foundation/child.rs": """
                        use super::Parent;
                    """,
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 0, output)

    def test_use_crate_alias_and_grouped_imports(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/action/mod.rs": """
                        use crate::{foundation::Value, foundation::Value as V};
                        pub struct Action;
                    """,
                    "src/foundation/mod.rs": "pub struct Value;\n",
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 0, output)

    def test_use_package_name_is_treated_as_crate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/action/mod.rs": """
                        use axiom_rs::foundation::Value;
                    """,
                    "src/foundation/mod.rs": "pub struct Value;\n",
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 0, output)

    def test_unassigned_source_file_is_violation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": LAYERS,
                    "Cargo.toml": '[package]\nname = "axiom-rs"\nversion = "0.1.0"\n',
                    "src/orphan.rs": "pub fn orphan() {}\n",
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 1, output)
        self.assertIn("src/orphan.rs", output.replace("\\", "/"))
        self.assertIn("unassigned", output.lower())

    def test_missing_config_is_tool_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            code, output = check_architecture.run(
                ["--root", tmp, "--config", str(Path(tmp) / "missing.toml")]
            )

        self.assertEqual(code, 2, output)
        self.assertIn("missing", output.lower())

    def test_invalid_config_is_tool_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(root, {"architecture.toml": "[layer.foundation\n"})
            code, output = run_in(root)

        self.assertEqual(code, 2, output)
        self.assertIn("invalid", output.lower())

    def test_unknown_layer_in_may_depend_on_is_tool_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": """
                        [layer.foundation]
                        paths = ["src/foundation/**"]
                        may_depend_on = ["missing"]
                    """,
                    "src/foundation/mod.rs": "pub struct Value;\n",
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 2, output)
        self.assertIn("missing", output.lower())

    def test_overlapping_layers_are_tool_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": """
                        [layer.one]
                        paths = ["src/foundation/**"]
                        may_depend_on = []

                        [layer.two]
                        paths = ["src/foundation/**"]
                        may_depend_on = []
                    """,
                    "src/foundation/mod.rs": "pub struct Value;\n",
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 2, output)
        self.assertIn("overlap", output.lower())

    def test_layer_cycle_in_config_is_tool_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                {
                    "architecture.toml": """
                        [layer.a]
                        paths = ["src/a/**"]
                        may_depend_on = ["b"]

                        [layer.b]
                        paths = ["src/b/**"]
                        may_depend_on = ["a"]
                    """,
                    "src/a/mod.rs": "",
                    "src/b/mod.rs": "",
                },
            )
            code, output = run_in(root)

        self.assertEqual(code, 2, output)
        self.assertIn("cycle", output.lower())


if __name__ == "__main__":
    unittest.main()
