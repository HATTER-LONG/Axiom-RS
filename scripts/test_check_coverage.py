"""Tests for scripts/check_coverage.py."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS_DIR))

import check_coverage  # noqa: E402


def llvm_report(
    *,
    lines: float = 94.2,
    regions: float = 92.7,
    branches: float = 91.3,
    files: list[dict] | None = None,
    functions: list[dict] | None = None,
) -> dict:
    export = {
        "type": "llvm.coverage.json.export",
        "version": "2.0.1",
        "data": [
            {
                "files": files or [],
                "totals": {
                    "lines": {"count": 100, "covered": 94, "percent": lines},
                    "regions": {"count": 100, "covered": 92, "percent": regions},
                    "branches": {"count": 100, "covered": 91, "percent": branches},
                },
            }
        ],
    }
    if functions is not None:
        export["data"][0]["functions"] = functions
    return export


class CheckCoverageTests(unittest.TestCase):
    def _write_report(self, directory: Path, payload: dict, name: str = "coverage.json") -> Path:
        path = directory / name
        path.write_text(json.dumps(payload), encoding="utf-8")
        return path

    def test_pass_when_all_thresholds_met(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = self._write_report(Path(tmp), llvm_report())
            code, output = check_coverage.run(
                [
                    str(report),
                    "--lines",
                    "90",
                    "--regions",
                    "90",
                    "--branches",
                    "90",
                ]
            )

        self.assertEqual(code, 0)
        self.assertIn("Lines:    94.2%  >= 90%  PASS", output)
        self.assertIn("Regions:  92.7%  >= 90%  PASS", output)
        self.assertIn("Branches: 91.3%  >= 90%  PASS", output)
        self.assertIn("Coverage check passed", output)

    def test_fail_when_branch_coverage_below_threshold(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = self._write_report(
                Path(tmp),
                llvm_report(lines=94.2, regions=92.7, branches=86.4),
            )
            code, output = check_coverage.run(
                [
                    str(report),
                    "--lines",
                    "90",
                    "--regions",
                    "90",
                    "--branches",
                    "90",
                ]
            )

        self.assertEqual(code, 1)
        self.assertIn("Lines:    94.2%  >= 90%  PASS", output)
        self.assertIn("Regions:  92.7%  >= 90%  PASS", output)
        self.assertIn("Branches: 86.4%  >= 90%  FAIL", output)
        self.assertIn("Coverage check failed:", output)
        self.assertIn("branch coverage below threshold", output)

    def test_exact_threshold_is_pass(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = self._write_report(
                Path(tmp),
                llvm_report(lines=90.0, regions=90.0, branches=90.0),
            )
            code, output = check_coverage.run(
                [
                    str(report),
                    "--lines",
                    "90",
                    "--regions",
                    "90",
                    "--branches",
                    "90",
                ]
            )

        self.assertEqual(code, 0)
        self.assertIn("Coverage check passed", output)
        self.assertNotIn("Uncovered regions:", output)

    def test_fail_lists_uncovered_regions_from_json(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / "src"
            src.mkdir()
            source = src / "lib.rs"
            source.write_text(
                "\n".join(
                    [
                        "pub fn add(left: u64, right: u64) -> u64 {",
                        "    left + right",
                        "}",
                        "pub fn unused() {",
                        "    let x = 1;",
                        "}",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            payload = llvm_report(
                lines=50.0,
                regions=50.0,
                files=[{"filename": str(source), "summary": {}}],
                functions=[
                    {
                        "name": "unused",
                        "filenames": [str(source)],
                        "regions": [
                            [1, 1, 3, 2, 4, 0, 0, 0],
                            [4, 1, 6, 2, 0, 0, 0, 0],
                            [4, 1, 6, 2, 0, 0, 0, 2],
                        ],
                    }
                ],
            )
            report = self._write_report(root, payload)
            code, output = check_coverage.run(
                [
                    str(report),
                    "--lines",
                    "90",
                    "--regions",
                    "90",
                    "--source-root",
                    str(src),
                ]
            )

        self.assertEqual(code, 1, output)
        self.assertIn("Uncovered regions:", output)
        self.assertIn("[0] start_line", output)
        self.assertIn("[4] count", output)
        self.assertIn("[7] kind", output)
        self.assertIn("src/lib.rs", output.replace("\\", "/"))
        self.assertIn("[4, 1, 6, 2, 0, 0, 0, 0]", output)
        self.assertIn("start 4:1", output)
        self.assertIn("end 6:2", output)
        self.assertIn("count 0", output)
        self.assertIn("kind code", output)
        self.assertIn("pub fn unused()", output)
        self.assertNotIn("kind skipped", output)

    def test_pass_does_not_list_uncovered_regions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / "src"
            src.mkdir()
            source = src / "lib.rs"
            source.write_text("pub fn add(a: u64, b: u64) -> u64 { a + b }\n", encoding="utf-8")
            payload = llvm_report(
                lines=94.2,
                regions=92.7,
                functions=[
                    {
                        "name": "unused",
                        "filenames": [str(source)],
                        "regions": [[1, 1, 1, 40, 0, 0, 0, 0]],
                    }
                ],
            )
            report = self._write_report(root, payload)
            code, output = check_coverage.run(
                [str(report), "--lines", "90", "--regions", "90"]
            )

        self.assertEqual(code, 0, output)
        self.assertNotIn("Uncovered regions:", output)

    def test_missing_report_is_tool_error(self) -> None:
        code, output = check_coverage.run(
            [
                str(Path("missing-coverage.json")),
                "--lines",
                "90",
            ]
        )

        self.assertEqual(code, 2)
        self.assertIn("missing-coverage.json", output)
        self.assertNotIn("Coverage check passed", output)

    def test_invalid_json_is_tool_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "coverage.json"
            path.write_text("{not json", encoding="utf-8")
            code, output = check_coverage.run([str(path), "--lines", "90"])

        self.assertEqual(code, 2)
        self.assertIn("invalid", output.lower())

    def test_missing_totals_is_tool_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = self._write_report(Path(tmp), {"data": [{"files": []}]})
            code, output = check_coverage.run([str(report), "--lines", "90"])

        self.assertEqual(code, 2)
        self.assertIn("totals", output.lower())

    def test_missing_requested_metric_is_tool_error(self) -> None:
        payload = llvm_report()
        del payload["data"][0]["totals"]["branches"]
        with tempfile.TemporaryDirectory() as tmp:
            report = self._write_report(Path(tmp), payload)
            code, output = check_coverage.run(
                [str(report), "--lines", "90", "--branches", "90"]
            )

        self.assertEqual(code, 2)
        self.assertIn("branch", output.lower())

    def test_invalid_threshold_is_tool_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = self._write_report(Path(tmp), llvm_report())
            code, output = check_coverage.run([str(report), "--lines", "101"])

        self.assertEqual(code, 2)
        self.assertIn("threshold", output.lower())

    def test_uninstrumented_branches_are_tool_error(self) -> None:
        payload = llvm_report(branches=0.0)
        payload["data"][0]["totals"]["branches"] = {
            "count": 0,
            "covered": 0,
            "percent": 0.0,
        }
        with tempfile.TemporaryDirectory() as tmp:
            report = self._write_report(Path(tmp), payload)
            code, output = check_coverage.run(
                [str(report), "--lines", "90", "--branches", "90"]
            )

        self.assertEqual(code, 2)
        self.assertIn("not collected", output.lower())
        self.assertIn("nightly", output.lower())

    def test_omitted_metric_is_not_checked(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = self._write_report(
                Path(tmp),
                llvm_report(lines=95.0, regions=10.0, branches=10.0),
            )
            code, output = check_coverage.run([str(report), "--lines", "90"])

        self.assertEqual(code, 0)
        self.assertIn("Lines:", output)
        self.assertNotIn("Regions:", output)
        self.assertNotIn("Branches:", output)

    def test_integrity_fails_when_production_file_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / "src"
            src.mkdir()
            (src / "lib.rs").write_text("pub fn x() {}", encoding="utf-8")
            (src / "runtime").mkdir()
            (src / "runtime" / "dispatcher.rs").write_text("pub fn y() {}", encoding="utf-8")
            (src / "action").mkdir()
            (src / "action" / "registry.rs").write_text("pub fn z() {}", encoding="utf-8")

            payload = llvm_report(
                files=[{"filename": str(src / "lib.rs"), "summary": {}}]
            )
            report = self._write_report(root, payload)

            code, output = check_coverage.run(
                [
                    str(report),
                    "--lines",
                    "90",
                    "--source-root",
                    str(src),
                    "--require-all-sources",
                ]
            )

        self.assertEqual(code, 1)
        self.assertIn("Coverage integrity failed:", output)
        self.assertIn("Missing production source files:", output)
        self.assertIn("src/runtime/dispatcher.rs", output.replace("\\", "/"))
        self.assertIn("src/action/registry.rs", output.replace("\\", "/"))

    def test_integrity_exclude_skips_configured_paths(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / "src"
            (src / "generated").mkdir(parents=True)
            (src / "lib.rs").write_text("pub fn x() {}", encoding="utf-8")
            (src / "generated" / "bindings.rs").write_text("pub fn g() {}", encoding="utf-8")

            exclude = root / "coverage.toml"
            exclude.write_text(
                'exclude = ["src/generated/**"]\n',
                encoding="utf-8",
            )
            payload = llvm_report(
                files=[{"filename": str(src / "lib.rs"), "summary": {}}]
            )
            report = self._write_report(root, payload)

            code, output = check_coverage.run(
                [
                    str(report),
                    "--lines",
                    "90",
                    "--source-root",
                    str(src),
                    "--require-all-sources",
                    "--exclude-config",
                    str(exclude),
                ]
            )

        self.assertEqual(code, 0)
        self.assertIn("Coverage check passed", output)
        self.assertNotIn("generated", output)

    def test_integrity_exclude_cli_glob(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / "src"
            (src / "platform").mkdir(parents=True)
            (src / "lib.rs").write_text("pub fn x() {}", encoding="utf-8")
            (src / "platform" / "win.rs").write_text("pub fn w() {}", encoding="utf-8")
            payload = llvm_report(
                files=[{"filename": str(src / "lib.rs"), "summary": {}}]
            )
            report = self._write_report(root, payload)

            code, output = check_coverage.run(
                [
                    str(report),
                    "--lines",
                    "90",
                    "--source-root",
                    str(src),
                    "--require-all-sources",
                    "--exclude",
                    "src/platform/**",
                ]
            )

        self.assertEqual(code, 0)
        self.assertIn("Coverage check passed", output)


if __name__ == "__main__":
    unittest.main()
