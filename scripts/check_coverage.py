#!/usr/bin/env python3
"""Validate a cargo-llvm-cov JSON report against coverage thresholds.

This script does not run tests or invoke cargo llvm-cov. cargo-make (or the
caller) generates the report; this tool only checks facts in that report.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence

EXIT_PASS = 0
EXIT_FAIL = 1
EXIT_ERROR = 2

METRIC_ORDER = ("lines", "regions", "branches")
METRIC_LABEL = {
    "lines": "Lines",
    "regions": "Regions",
    "branches": "Branches",
}
METRIC_SINGULAR = {
    "lines": "line",
    "regions": "region",
    "branches": "branch",
}

# llvm-cov region: [LineStart, ColumnStart, LineEnd, ColumnEnd, Count, FileID, ExpandedFileID, Kind]
REGION_KIND = {
    0: "code",
    1: "expansion",
    2: "skipped",
    3: "gap",
    4: "branch",
    5: "mcdc_decision",
    6: "mcdc_branch",
}
UNCOVERED_REGION_KINDS = {0}
REGION_LAYOUT = (
    (0, "start_line", "first line of the region (1-based)"),
    (1, "start_column", "first column of the region (1-based)"),
    (2, "end_line", "last line of the region (1-based)"),
    (3, "end_column", "last column of the region (1-based)"),
    (4, "count", "execution count; 0 means this region was never executed"),
    (5, "file_id", "index into the function's filenames list"),
    (6, "expanded_file_id", "file id after macro or include expansion"),
    (7, "kind", "0=code 1=expansion 2=skipped 3=gap 4=branch 5=mcdc_decision 6=mcdc_branch"),
)
MAX_UNCOVERED_FILES = 40
MAX_UNCOVERED_REGIONS_PER_FILE = 30
MAX_SOURCE_LINES_PER_REGION = 12


class ToolError(Exception):
    """Invalid input, configuration, or report shape."""


@dataclass(frozen=True)
class Metric:
    name: str
    percent: float
    count: int | None = None


@dataclass(frozen=True)
class MetricResult:
    name: str
    percent: float
    threshold: float
    passed: bool

    @property
    def label(self) -> str:
        return METRIC_LABEL[self.name]


@dataclass(frozen=True)
class UncoveredRegion:
    filename: str
    raw: tuple[int, ...]
    start_line: int
    start_column: int
    end_line: int
    end_column: int
    count: int
    file_id: int
    expanded_file_id: int
    kind: int
    source_lines: tuple[tuple[int, str], ...] = ()

    @property
    def kind_name(self) -> str:
        return REGION_KIND.get(self.kind, str(self.kind))


def run(argv: Sequence[str] | None = None) -> tuple[int, str]:
    try:
        args = _parse_args(argv)
        args.lines = _parse_threshold("lines", args.lines)
        args.regions = _parse_threshold("regions", args.regions)
        args.branches = _parse_threshold("branches", args.branches)
        report_path = Path(args.report)
        report = load_report(report_path)
        totals = extract_totals(report)
        thresholds = {
            "lines": args.lines,
            "regions": args.regions,
            "branches": args.branches,
        }
        if not any(value is not None for value in thresholds.values()):
            raise ToolError("at least one of --lines, --regions, --branches is required")

        results = check_thresholds(totals, thresholds)
        threshold_failed = [item for item in results if not item.passed]

        missing: list[str] = []
        source_root = Path(args.source_root) if args.source_root else None
        if args.require_all_sources:
            integrity_root = source_root or Path("src")
            excludes = list(args.exclude or [])
            if args.exclude_config:
                excludes.extend(load_excludes(Path(args.exclude_config)))
            missing = find_missing_sources(
                report,
                source_root=integrity_root,
                excludes=excludes,
            )

        uncovered: list[UncoveredRegion] = []
        if threshold_failed:
            uncovered = collect_uncovered_regions(
                report,
                project_root=_project_root(report_path, source_root),
            )

        output = format_coverage_output(results, missing, uncovered)
        if threshold_failed or missing:
            return EXIT_FAIL, output
        return EXIT_PASS, output
    except ToolError as exc:
        return EXIT_ERROR, f"Coverage check error: {exc}"
    except Exception as exc:  # pragma: no cover - unexpected tool failure
        return EXIT_ERROR, f"Coverage check error: {exc}"


def main(argv: Sequence[str] | None = None) -> int:
    code, output = run(argv)
    stream = sys.stdout if code == EXIT_PASS else sys.stderr
    if code == EXIT_FAIL:
        stream = sys.stdout
    print(output, file=stream)
    return code


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate cargo-llvm-cov JSON coverage thresholds.",
    )
    parser.add_argument("report", help="Path to cargo llvm-cov JSON output")
    parser.add_argument("--lines", default=None, help="Minimum line coverage percent")
    parser.add_argument("--regions", default=None, help="Minimum region coverage percent")
    parser.add_argument("--branches", default=None, help="Minimum branch coverage percent")
    parser.add_argument(
        "--source-root",
        default=None,
        help="Production source directory for coverage integrity (default: src)",
    )
    parser.add_argument(
        "--require-all-sources",
        action="store_true",
        help="Fail if any production source file is missing from the report",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        help="Glob relative to the project root to skip during integrity checks",
    )
    parser.add_argument(
        "--exclude-config",
        default=None,
        help="TOML file with an exclude = [...] list for integrity checks",
    )
    try:
        return parser.parse_args(argv)
    except SystemExit as exc:
        if exc.code in (0, None):
            raise
        raise ToolError("invalid command line arguments") from exc


def _parse_threshold(name: str, raw: str | None) -> float | None:
    if raw is None:
        return None
    try:
        value = float(raw)
    except (TypeError, ValueError) as exc:
        raise ToolError(f"invalid {name} threshold: {raw}") from exc
    if value < 0 or value > 100:
        raise ToolError(f"{name} threshold must be between 0 and 100: {raw}")
    return value


def load_report(path: Path) -> dict[str, Any]:
    if not path.is_file():
        raise ToolError(f"missing coverage report: {path}")
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ToolError(f"cannot read coverage report: {path}") from exc
    try:
        payload = json.loads(text)
    except json.JSONDecodeError as exc:
        raise ToolError(f"invalid coverage JSON: {path}") from exc
    if not isinstance(payload, dict):
        raise ToolError(f"invalid coverage JSON: expected an object in {path}")
    return payload


def extract_totals(report: dict[str, Any]) -> dict[str, Metric]:
    totals = _find_totals(report)
    metrics: dict[str, Metric] = {}
    for name in METRIC_ORDER:
        item = totals.get(name)
        if not isinstance(item, dict) or "percent" not in item:
            continue
        try:
            percent = float(item["percent"])
        except (TypeError, ValueError) as exc:
            raise ToolError(f"invalid {name} coverage percent") from exc
        count: int | None = None
        if "count" in item:
            try:
                count = int(item["count"])
            except (TypeError, ValueError) as exc:
                raise ToolError(f"invalid {name} coverage count") from exc
        metrics[name] = Metric(name=name, percent=percent, count=count)
    if not metrics:
        raise ToolError("coverage report is missing totals")
    return metrics


def _find_totals(report: dict[str, Any]) -> dict[str, Any]:
    if isinstance(report.get("totals"), dict):
        return report["totals"]
    data = report.get("data")
    if isinstance(data, list):
        for entry in data:
            if isinstance(entry, dict) and isinstance(entry.get("totals"), dict):
                return entry["totals"]
    raise ToolError("coverage report is missing totals")


def check_thresholds(
    totals: dict[str, Metric],
    thresholds: dict[str, float | None],
) -> list[MetricResult]:
    results: list[MetricResult] = []
    for name in METRIC_ORDER:
        threshold = thresholds.get(name)
        if threshold is None:
            continue
        metric = totals.get(name)
        if metric is None:
            raise ToolError(f"coverage report is missing {name} totals")
        if name == "branches" and metric.count == 0:
            raise ToolError(
                "branch coverage was not collected; "
                "cargo llvm-cov --branch requires the nightly toolchain"
            )
        results.append(
            MetricResult(
                name=name,
                percent=metric.percent,
                threshold=threshold,
                passed=metric.percent + 1e-9 >= threshold,
            )
        )
    return results


def load_excludes(path: Path) -> list[str]:
    if not path.is_file():
        raise ToolError(f"missing exclude config: {path}")
    try:
        payload = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ToolError(f"invalid exclude config: {path}") from exc
    exclude = payload.get("exclude", [])
    if not isinstance(exclude, list) or any(not isinstance(item, str) for item in exclude):
        raise ToolError(f"exclude config must contain exclude = [string, ...]: {path}")
    return exclude


def _project_root(report_path: Path, source_root: Path | None) -> Path:
    if source_root is not None:
        resolved = source_root.resolve()
        return resolved.parent if resolved.name == "src" else resolved
    parent = report_path.resolve().parent
    if parent.name == "target":
        return parent.parent
    return Path.cwd()


def collect_uncovered_regions(
    report: dict[str, Any],
    *,
    project_root: Path,
) -> list[UncoveredRegion]:
    found: list[UncoveredRegion] = []
    seen: set[tuple[str, tuple[int, ...]]] = set()

    for filename, raw in _iter_named_regions(report):
        parsed = parse_uncovered_region(raw, filename, project_root)
        if parsed is None:
            continue
        key = (parsed.filename, parsed.raw)
        if key in seen:
            continue
        seen.add(key)
        found.append(parsed)

    found.sort(key=lambda item: (item.filename, item.start_line, item.start_column, item.end_line))
    return found


def _iter_named_regions(report: dict[str, Any]) -> Iterable[tuple[str, list[Any]]]:
    for entry in _coverage_entries(report):
        files = entry.get("files", [])
        if isinstance(files, list):
            for item in files:
                if not isinstance(item, dict):
                    continue
                filename = item.get("filename")
                regions = item.get("regions", [])
                if isinstance(filename, str) and isinstance(regions, list):
                    for region in regions:
                        if isinstance(region, list):
                            yield filename, region

        functions = entry.get("functions", [])
        if isinstance(functions, list):
            for function in functions:
                if not isinstance(function, dict):
                    continue
                filenames = function.get("filenames", [])
                regions = function.get("regions", [])
                if not isinstance(filenames, list) or not isinstance(regions, list):
                    continue
                names = [name for name in filenames if isinstance(name, str)]
                for region in regions:
                    if not isinstance(region, list):
                        continue
                    filename = _region_filename(names, region)
                    if filename:
                        yield filename, region


def _coverage_entries(report: dict[str, Any]) -> list[dict[str, Any]]:
    data = report.get("data")
    if isinstance(data, list):
        return [entry for entry in data if isinstance(entry, dict)]
    return [report]


def _region_filename(filenames: list[str], region: list[Any]) -> str | None:
    if not filenames:
        return None
    file_id = 0
    if len(region) > 5:
        try:
            file_id = int(region[5])
        except (TypeError, ValueError):
            file_id = 0
    if 0 <= file_id < len(filenames):
        return filenames[file_id]
    return filenames[0]


def parse_uncovered_region(
    raw: list[Any],
    filename: str,
    project_root: Path,
) -> UncoveredRegion | None:
    if len(raw) < 5:
        return None
    try:
        values = [int(item) for item in raw[:8]]
    except (TypeError, ValueError):
        return None
    while len(values) < 8:
        values.append(0)
    count = values[4]
    kind = values[7]
    if count != 0 or kind not in UNCOVERED_REGION_KINDS:
        return None

    relative = _normalize_coverage_filename(filename, project_root)
    if not _is_project_source(relative):
        return None

    source_path = Path(filename)
    if not source_path.is_file():
        candidate = project_root / relative
        source_path = candidate if candidate.is_file() else source_path

    return UncoveredRegion(
        filename=relative.replace("\\", "/"),
        raw=tuple(values),
        start_line=values[0],
        start_column=values[1],
        end_line=values[2],
        end_column=values[3],
        count=values[4],
        file_id=values[5],
        expanded_file_id=values[6],
        kind=values[7],
        source_lines=_read_source_lines(source_path, values[0], values[2]),
    )


def _is_project_source(relative: str) -> bool:
    posix = relative.replace("\\", "/")
    return posix.startswith("src/") or posix.startswith("tests/") or "/src/" in posix


def _read_source_lines(path: Path, start: int, end: int) -> tuple[tuple[int, str], ...]:
    if not path.is_file() or start < 1:
        return ()
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return ()
    last = min(max(end, start), len(lines), start + MAX_SOURCE_LINES_PER_REGION - 1)
    snippet: list[tuple[int, str]] = []
    for number in range(start, last + 1):
        snippet.append((number, lines[number - 1]))
    return tuple(snippet)


def find_missing_sources(
    report: dict[str, Any],
    *,
    source_root: Path,
    excludes: Iterable[str] = (),
) -> list[str]:
    if not source_root.exists():
        raise ToolError(f"missing source root: {source_root}")

    project_root = source_root.parent if source_root.name == "src" else source_root
    covered = _covered_paths(report, project_root)
    exclude_patterns = list(excludes)
    missing: list[str] = []
    for path in sorted(source_root.rglob("*.rs")):
        relative = _relative_posix(path, project_root)
        if _matches_any(relative, exclude_patterns):
            continue
        if not _is_covered(relative, covered):
            missing.append(relative)
    return missing


def _covered_paths(report: dict[str, Any], project_root: Path) -> set[str]:
    names: set[str] = set()
    for filename in _iter_filenames(report):
        names.add(_normalize_coverage_filename(filename, project_root))
    return names


def _iter_filenames(report: dict[str, Any]) -> Iterable[str]:
    data = report.get("data")
    entries: list[Any]
    if isinstance(data, list):
        entries = data
    else:
        entries = [report]
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        files = entry.get("files", [])
        if not isinstance(files, list):
            continue
        for item in files:
            if isinstance(item, dict) and isinstance(item.get("filename"), str):
                yield item["filename"]


def _normalize_coverage_filename(filename: str, project_root: Path) -> str:
    posix = filename.replace("\\", "/")
    root_posix = project_root.resolve().as_posix()
    candidate = Path(filename)
    try:
        return candidate.resolve().relative_to(project_root.resolve()).as_posix()
    except (OSError, ValueError):
        pass
    if posix.lower().startswith(root_posix.lower() + "/"):
        return posix[len(root_posix) + 1 :]
    marker = "/src/"
    index = posix.lower().rfind(marker)
    if index >= 0:
        return posix[index + 1 :]
    if posix.startswith("src/"):
        return posix
    return posix


def _relative_posix(path: Path, project_root: Path) -> str:
    try:
        return path.resolve().relative_to(project_root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def _is_covered(relative: str, covered: set[str]) -> bool:
    if relative in covered:
        return True
    return any(item.replace("\\", "/").endswith(relative) for item in covered)


def _matches_any(path: str, patterns: Iterable[str]) -> bool:
    return any(glob_match(path, pattern) for pattern in patterns)


def glob_match(path: str, pattern: str) -> bool:
    path = path.replace("\\", "/")
    pattern = pattern.replace("\\", "/")
    if pattern.endswith("/**"):
        prefix = pattern[:-3]
        return path == prefix or path.startswith(prefix + "/")
    regex = _glob_to_regex(pattern)
    return re.match(regex, path) is not None


def _glob_to_regex(pattern: str) -> str:
    parts: list[str] = ["^"]
    i = 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            parts.append("(?:.*/)?")
            i += 3
        elif pattern.startswith("**", i):
            parts.append(".*")
            i += 2
        elif pattern[i] == "*":
            parts.append("[^/]*")
            i += 1
        elif pattern[i] == "?":
            parts.append("[^/]")
            i += 1
        else:
            parts.append(re.escape(pattern[i]))
            i += 1
    parts.append("$")
    return "".join(parts)


def format_coverage_output(
    results: list[MetricResult],
    missing: list[str],
    uncovered: list[UncoveredRegion] | None = None,
) -> str:
    lines = ["Coverage:", ""]
    for item in results:
        status = "PASS" if item.passed else "FAIL"
        label = f"{item.label}:"
        lines.append(
            f"{label:<10}{item.percent:.1f}%  >= {item.threshold:g}%  {status}"
        )

    if missing:
        if results:
            lines.append("")
        lines.append("Coverage integrity failed:")
        lines.append("")
        lines.append("Missing production source files:")
        for path in missing:
            lines.append(f"  {path}")
        return "\n".join(lines)

    if any(not item.passed for item in results):
        lines.append("")
        lines.append("Coverage check failed:")
        for item in results:
            if not item.passed:
                lines.append(f"{METRIC_SINGULAR[item.name]} coverage below threshold")
        if uncovered:
            lines.append("")
            lines.extend(_format_uncovered_regions(uncovered))
        return "\n".join(lines)

    lines.append("")
    lines.append("Coverage check passed")
    return "\n".join(lines)


def _format_uncovered_regions(uncovered: list[UncoveredRegion]) -> list[str]:
    grouped: dict[str, list[UncoveredRegion]] = {}
    for item in uncovered:
        grouped.setdefault(item.filename, []).append(item)

    lines = [
        "Uncovered regions:",
        "",
        "Each region is [start_line, start_column, end_line, end_column, count, file_id, expanded_file_id, kind]",
    ]
    for index, name, description in REGION_LAYOUT:
        lines.append(f"  [{index}] {name:<18} {description}")
    lines.append("")

    file_names = list(grouped)
    shown_files = file_names[:MAX_UNCOVERED_FILES]
    for filename in shown_files:
        regions = grouped[filename][:MAX_UNCOVERED_REGIONS_PER_FILE]
        extra_regions = len(grouped[filename]) - len(regions)
        lines.append(filename)
        for region in regions:
            raw = "[" + ", ".join(str(value) for value in region.raw) + "]"
            lines.append(f"  {raw}")
            lines.append(
                "    "
                f"start {region.start_line}:{region.start_column}  "
                f"end {region.end_line}:{region.end_column}  "
                f"count {region.count}  "
                f"file_id {region.file_id}  "
                f"expanded_file_id {region.expanded_file_id}  "
                f"kind {region.kind_name}"
            )
            for number, text in region.source_lines:
                lines.append(f"    {number:>4} | {text}")
            if region.source_lines:
                omitted = region.end_line - region.start_line + 1 - len(region.source_lines)
                if omitted > 0:
                    lines.append(f"    ... {omitted} more lines in this region")
        if extra_regions > 0:
            lines.append(f"  ... {extra_regions} more uncovered regions")
        lines.append("")

    extra_files = len(file_names) - len(shown_files)
    if extra_files > 0:
        lines.append(f"... {extra_files} more files with uncovered regions")
    return lines


if __name__ == "__main__":
    sys.exit(main())
