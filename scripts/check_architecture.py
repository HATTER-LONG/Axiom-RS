#!/usr/bin/env python3
"""Validate Axiom-RS source against architecture.toml layer rules.

This script only checks static dependency facts. It does not format code,
install tools, or modify the tree. cargo-make remains the orchestrator.

Supported crate-local references:

- `use crate::...` / `use <package>::...` trees, including groups and `as`
- `mod` declarations
- fully qualified `crate::...` and package-name paths in types, signatures,
  and expressions

Not a compiler. The following are out of scope and may be missed:

- macro-generated paths
- type aliases and re-exports that hide the original module
- `use` aliases referenced by the short name
- `include!` and generated files outside scanned paths
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable, Sequence

EXIT_PASS = 0
EXIT_FAIL = 1
EXIT_ERROR = 2

IDENT = r"[A-Za-z_][A-Za-z0-9_]*"
USE_HEAD = re.compile(
    rf"(?m)^[ \t]*(?:pub(?:\s*\([^)]*\))?\s+)?use\s+"
)
MOD_DECL = re.compile(
    rf"(?m)^[ \t]*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+({IDENT})\s*;"
)
MOD_BLOCK = re.compile(
    rf"(?m)^[ \t]*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+({IDENT})\s*\{{"
)
AS_ALIAS = re.compile(rf"\s+as\s+{IDENT}\s*$")


class ToolError(Exception):
    """Invalid configuration, missing input, or unparseable source."""


@dataclass(frozen=True)
class Layer:
    name: str
    paths: tuple[str, ...]
    may_depend_on: tuple[str, ...]
    internal_modules: tuple[str, ...]
    crates: tuple[str, ...]


@dataclass(frozen=True)
class Rule:
    from_spec: str
    to_spec: str


@dataclass
class Config:
    layers: dict[str, Layer]
    allow: list[Rule] = field(default_factory=list)
    deny: list[Rule] = field(default_factory=list)
    include_tests: bool = False


@dataclass(frozen=True)
class Edge:
    source_file: str
    source_line: int
    snippet: str
    from_module: str
    from_layer: str
    to_module: str
    to_layer: str | None
    for_cycles: bool


@dataclass(frozen=True)
class Violation:
    file: str
    title: str
    detail: str
    snippet: str


def run(argv: Sequence[str] | None = None) -> tuple[int, str]:
    try:
        args = _parse_args(argv)
        root = Path(args.root).resolve()
        config_path = Path(args.config)
        if not config_path.is_absolute():
            candidate = root / config_path
            config_path = candidate if candidate.is_file() or not Path(args.config).is_file() else Path(args.config)
        config = load_config(config_path)
        crate_name = load_crate_name(root)
        crate_layers = crate_layer_map(config)
        files = collect_files(root, include_tests=config.include_tests, config=config)
        file_layers = assign_layers(files, root, config)
        edges: list[Edge] = []
        for path in files:
            relative = _relative_posix(path, root)
            if relative.startswith("tests/"):
                continue
            layer = file_layers.get(relative)
            if layer is None:
                continue
            edges.extend(
                parse_file_edges(
                    path,
                    relative=relative,
                    layer=layer,
                    crate_name=crate_name,
                    crate_layers=crate_layers,
                    file_layers=file_layers,
                    config=config,
                    root=root,
                )
            )
        violations = collect_violations(files, root, file_layers, edges, config)
        output = format_output(files, edges, violations)
        return (EXIT_PASS, output) if not violations else (EXIT_FAIL, output)
    except ToolError as exc:
        return EXIT_ERROR, f"Architecture check error: {exc}"
    except Exception as exc:  # pragma: no cover - unexpected tool failure
        return EXIT_ERROR, f"Architecture check error: {exc}"


def main(argv: Sequence[str] | None = None) -> int:
    code, output = run(argv)
    print(output, file=sys.stdout if code != EXIT_ERROR else sys.stderr)
    return code


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate architecture boundaries.")
    parser.add_argument("--root", default=".", help="Project root directory")
    parser.add_argument("--config", default="architecture.toml", help="Architecture rules file")
    try:
        return parser.parse_args(argv)
    except SystemExit as exc:
        if exc.code in (0, None):
            raise
        raise ToolError("invalid command line arguments") from exc


def load_config(path: Path) -> Config:
    if not path.is_file():
        raise ToolError(f"missing configuration file: {path}")
    try:
        payload = tomllib.loads(path.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as exc:
        raise ToolError(f"invalid configuration: {path}") from exc
    except OSError as exc:
        raise ToolError(f"cannot read configuration: {path}") from exc

    raw_layers = payload.get("layer")
    if not isinstance(raw_layers, dict) or not raw_layers:
        raise ToolError("configuration must define at least one [layer.*] table")

    layers: dict[str, Layer] = {}
    for name, raw in raw_layers.items():
        if not isinstance(raw, dict):
            raise ToolError(f"invalid layer: {name}")
        paths = raw.get("paths")
        if not isinstance(paths, list) or not paths or any(not isinstance(item, str) for item in paths):
            raise ToolError(f"layer {name} must have paths = [string, ...]")
        may_depend_on = raw.get("may_depend_on", [])
        if not isinstance(may_depend_on, list) or any(not isinstance(item, str) for item in may_depend_on):
            raise ToolError(f"layer {name} may_depend_on must be a list of strings")
        internal_modules = raw.get("internal_modules", [])
        if not isinstance(internal_modules, list) or any(not isinstance(item, str) for item in internal_modules):
            raise ToolError(f"layer {name} internal_modules must be a list of strings")
        crates = raw.get("crates", [])
        if not isinstance(crates, list) or any(not isinstance(item, str) for item in crates):
            raise ToolError(f"layer {name} crates must be a list of strings")
        layers[name] = Layer(
            name=name,
            paths=tuple(paths),
            may_depend_on=tuple(may_depend_on),
            internal_modules=tuple(internal_modules),
            crates=tuple(crates),
        )

    for layer in layers.values():
        for dep in layer.may_depend_on:
            if dep not in layers:
                raise ToolError(f"layer {layer.name} may_depend_on unknown layer: {dep}")

    cycle = _layer_config_cycle(layers)
    if cycle:
        raise ToolError("layer dependency cycle in configuration: " + " -> ".join(cycle))

    scan = payload.get("scan", {})
    if scan and not isinstance(scan, dict):
        raise ToolError("invalid [scan] table")
    include_tests = bool(scan.get("include_tests", False)) if isinstance(scan, dict) else False

    return Config(
        layers=layers,
        allow=_load_rules(payload.get("allow", []), "allow"),
        deny=_load_rules(payload.get("deny", []), "deny"),
        include_tests=include_tests,
    )


def _load_rules(raw: object, name: str) -> list[Rule]:
    if raw in (None, []):
        return []
    if not isinstance(raw, list):
        raise ToolError(f"[[{name}]] must be an array of tables")
    rules: list[Rule] = []
    for item in raw:
        if not isinstance(item, dict) or "from" not in item or "to" not in item:
            raise ToolError(f"[[{name}]] entries need from and to")
        if not isinstance(item["from"], str) or not isinstance(item["to"], str):
            raise ToolError(f"[[{name}]] from/to must be strings")
        rules.append(Rule(from_spec=item["from"], to_spec=item["to"]))
    return rules


def _layer_config_cycle(layers: dict[str, Layer]) -> list[str] | None:
    visiting: set[str] = set()
    seen: set[str] = set()
    stack: list[str] = []

    def dfs(node: str) -> list[str] | None:
        if node in visiting:
            return stack[stack.index(node) :] + [node]
        if node in seen:
            return None
        visiting.add(node)
        stack.append(node)
        for dep in layers[node].may_depend_on:
            found = dfs(dep)
            if found:
                return found
        stack.pop()
        visiting.remove(node)
        seen.add(node)
        return None

    for name in layers:
        found = dfs(name)
        if found:
            return found
    return None


def load_crate_name(root: Path) -> str:
    cargo = root / "Cargo.toml"
    if not cargo.is_file():
        return ""
    try:
        payload = tomllib.loads(cargo.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError):
        return ""
    name = payload.get("package", {}).get("name", "")
    if not isinstance(name, str):
        return ""
    return name.replace("-", "_")


def crate_layer_map(config: Config) -> dict[str, str]:
    mapping: dict[str, str] = {}
    for layer in config.layers.values():
        for crate in layer.crates:
            mapping[crate.replace("-", "_")] = layer.name
    return mapping


def collect_files(root: Path, *, include_tests: bool, config: Config | None = None) -> list[Path]:
    files: set[Path] = set()
    src = root / "src"
    if src.is_dir():
        files.update(path for path in src.rglob("*.rs") if path.is_file())
    if config is not None:
        for layer in config.layers.values():
            for pattern in layer.paths:
                files.update(_files_for_layer_pattern(root, pattern))
    if include_tests:
        tests = root / "tests"
        if tests.is_dir():
            files.update(path for path in tests.rglob("*.rs") if path.is_file())
    return sorted(files)


def _files_for_layer_pattern(root: Path, pattern: str) -> list[Path]:
    posix = pattern.replace("\\", "/")
    if posix.endswith("/**"):
        directory = root / posix[:-3]
        if directory.is_dir():
            return [path for path in directory.rglob("*.rs") if path.is_file()]
        sibling = Path(str(directory) + ".rs")
        return [sibling] if sibling.is_file() else []
    candidate = root / posix
    if candidate.is_file():
        return [candidate]
    return []


def assign_layers(files: list[Path], root: Path, config: Config) -> dict[str, str]:
    assigned: dict[str, str] = {}
    for path in files:
        relative = _relative_posix(path, root)
        if relative.startswith("tests/"):
            continue
        matches = [
            layer.name
            for layer in config.layers.values()
            if any(glob_match(relative, pattern) for pattern in layer.paths)
        ]
        if len(matches) > 1:
            raise ToolError(f"overlapping layers for {relative}: {', '.join(sorted(matches))}")
        if matches:
            assigned[relative] = matches[0]
    return assigned


def parse_file_edges(
    path: Path,
    *,
    relative: str,
    layer: str,
    crate_name: str,
    crate_layers: dict[str, str],
    file_layers: dict[str, str],
    config: Config,
    root: Path,
) -> list[Edge]:
    try:
        original = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ToolError(f"cannot read {relative}") from exc

    masked = mask_comments_and_strings(original)
    masked = blank_cfg_test_items(masked)
    file_module = file_module_path(relative)
    edges: list[Edge] = []
    seen_refs: set[tuple[int, str]] = set()

    for match in USE_HEAD.finditer(masked):
        start = match.end()
        end = _find_semicolon(masked, start)
        if end is None:
            raise ToolError(f"unparseable use statement in {relative}")
        tree = masked[start:end].strip()
        line_no = masked[: match.start()].count("\n") + 1
        snippet = _snippet(original, line_no, f"use {collapse_ws(tree)};")
        module_at = module_at_offset(masked, match.start(), file_module)
        try:
            targets = resolve_use_targets(tree, module_at, crate_name, crate_layers)
        except ValueError as exc:
            raise ToolError(f"unparseable use statement in {relative}: {exc}") from exc
        for to_module, to_layer_hint in targets:
            to_layer = to_layer_hint or layer_for_module(to_module, file_layers, config)
            edges.append(
                Edge(
                    source_file=relative,
                    source_line=line_no,
                    snippet=snippet,
                    from_module=module_at,
                    from_layer=layer,
                    to_module=to_module,
                    to_layer=to_layer,
                    for_cycles=True,
                )
            )

    for match in MOD_DECL.finditer(masked):
        child = match.group(1)
        line_no = masked[: match.start()].count("\n") + 1
        snippet = _snippet(original, line_no, f"mod {child};")
        module_at = module_at_offset(masked, match.start(), file_module)
        to_module = f"{module_at}::{child}" if module_at else child
        edges.append(
            Edge(
                source_file=relative,
                source_line=line_no,
                snippet=snippet,
                from_module=module_at,
                from_layer=layer,
                to_module=to_module,
                to_layer=layer_for_module(to_module, file_layers, config),
                for_cycles=False,
            )
        )

    use_blanked = blank_use_statements(masked)
    crate_alt = "crate|{name}".format(name=re.escape(crate_name)) if crate_name else "crate"
    crate_pattern = re.compile(
        r"\b((?:" + crate_alt + r")(?:::" + IDENT + r")+)"
    )
    for match in crate_pattern.finditer(use_blanked):
        raw = match.group(1)
        line_no = use_blanked[: match.start()].count("\n") + 1
        module_at = module_at_offset(masked, match.start(), file_module)
        try:
            resolved, hint = resolve_use_path(raw, module_at, crate_name, crate_layers)
        except ValueError:
            continue
        if resolved is None:
            continue
        key = (line_no, resolved)
        if key in seen_refs:
            continue
        seen_refs.add(key)
        snippet = _snippet(original, line_no, raw)
        to_layer = hint or layer_for_module(resolved, file_layers, config)
        edges.append(
            Edge(
                source_file=relative,
                source_line=line_no,
                snippet=snippet,
                from_module=module_at,
                from_layer=layer,
                to_module=resolved,
                to_layer=to_layer,
                for_cycles=True,
            )
        )
    return edges


def blank_use_statements(text: str) -> str:
    chars = list(text)
    for match in USE_HEAD.finditer(text):
        end = _find_semicolon(text, match.end())
        if end is None:
            continue
        for index in range(match.start(), end + 1):
            if chars[index] != "\n":
                chars[index] = " "
    return "".join(chars)


def collect_violations(
    files: list[Path],
    root: Path,
    file_layers: dict[str, str],
    edges: list[Edge],
    config: Config,
) -> list[Violation]:
    violations: list[Violation] = []
    for path in files:
        relative = _relative_posix(path, root)
        if relative.startswith("tests/"):
            continue
        if relative not in file_layers:
            violations.append(
                Violation(
                    file=relative,
                    title="unassigned file:",
                    detail="no layer matches this path",
                    snippet="",
                )
            )

    for edge in edges:
        if _matches_any_rule(config.deny, edge):
            violations.append(_forbidden(edge))
            continue
        allowed = _matches_any_rule(config.allow, edge)
        if edge.to_layer == "tests" or edge.to_module == "tests" or edge.to_module.startswith("tests::"):
            if not allowed:
                violations.append(
                    Violation(
                        file=edge.source_file,
                        title="forbidden dependency:",
                        detail="production -> tests",
                        snippet=edge.snippet,
                    )
                )
            continue
        internal = _internal_bypass(edge, config)
        if internal and not allowed:
            violations.append(
                Violation(
                    file=edge.source_file,
                    title="forbidden internal module access:",
                    detail=internal,
                    snippet=edge.snippet,
                )
            )
            continue
        if (
            edge.to_layer
            and edge.to_layer != edge.from_layer
            and edge.to_layer not in config.layers[edge.from_layer].may_depend_on
            and not allowed
        ):
            violations.append(_forbidden(edge))

    for cycle, edge in _module_cycles(edges):
        violations.append(
            Violation(
                file=edge.source_file,
                title="circular dependency:",
                detail=" -> ".join(cycle),
                snippet=edge.snippet,
            )
        )
    return violations


def _forbidden(edge: Edge) -> Violation:
    target = edge.to_layer or edge.to_module
    return Violation(
        file=edge.source_file,
        title="forbidden dependency:",
        detail=f"{edge.from_layer} -> {target}",
        snippet=edge.snippet,
    )


def _internal_bypass(edge: Edge, config: Config) -> str | None:
    for layer in config.layers.values():
        if layer.name == edge.from_layer:
            continue
        for internal in layer.internal_modules:
            if edge.to_module == internal or edge.to_module.startswith(internal + "::"):
                return internal
    return None


def _matches_any_rule(rules: Iterable[Rule], edge: Edge) -> bool:
    return any(_rule_matches(rule, edge) for rule in rules)


def _rule_matches(rule: Rule, edge: Edge) -> bool:
    from_ok = (
        rule.from_spec in {"*", edge.from_layer, edge.source_file}
        or glob_match(edge.source_file, rule.from_spec)
    )
    to_spec = rule.to_spec.removeprefix("crate::")
    to_ok = (
        rule.to_spec in {edge.to_layer, edge.to_module, "*"}
        or to_spec == edge.to_module
        or (edge.to_module and (edge.to_module == to_spec or edge.to_module.startswith(to_spec + "::")))
        or (edge.to_layer is not None and to_spec == edge.to_layer)
    )
    return from_ok and to_ok


def _module_cycles(edges: list[Edge]) -> list[tuple[list[str], Edge]]:
    graph: dict[str, list[Edge]] = {}
    for edge in edges:
        if not edge.for_cycles or not edge.to_module:
            continue
        source = _module_node(edge.from_module, edge.source_file)
        target = _type_stripped(edge.to_module)
        if not target or source == target:
            continue
        graph.setdefault(source, []).append(
            Edge(**{**edge.__dict__, "to_module": target})
        )

    found: list[tuple[list[str], Edge]] = []
    visiting: dict[str, Edge] = {}
    seen: set[str] = set()
    stack: list[str] = []

    def dfs(node: str) -> None:
        if node in visiting:
            cycle = stack[stack.index(node) :] + [node]
            found.append((cycle, visiting[node]))
            return
        if node in seen:
            return
        visiting[node] = visiting.get(node, Edge(
            source_file="",
            source_line=0,
            snippet="",
            from_module=node,
            from_layer="",
            to_module="",
            to_layer=None,
            for_cycles=True,
        ))
        stack.append(node)
        for edge in graph.get(node, []):
            visiting[node] = edge
            dfs(edge.to_module)
        stack.pop()
        visiting.pop(node, None)
        seen.add(node)

    for node in list(graph):
        dfs(node)
    return found


def _module_node(module: str, source_file: str) -> str:
    if module:
        return module
    return file_module_path(source_file)


def _type_stripped(module: str) -> str:
    parts = module.split("::")
    if parts and parts[-1][:1].isupper():
        parts = parts[:-1]
    return "::".join(parts)


def layer_for_module(module: str, file_layers: dict[str, str], config: Config) -> str | None:
    if not module:
        return file_layers.get("src/lib.rs") or file_layers.get("src/main.rs")
    if module == "tests" or module.startswith("tests::"):
        return "tests"
    stripped = _type_stripped(module)
    for candidate in _module_files(stripped):
        if candidate in file_layers:
            return file_layers[candidate]
    first = (stripped or module).split("::", 1)[0]
    probe = f"src/{first}/probe.rs"
    probe_file = f"src/{first}.rs"
    for layer in config.layers.values():
        if any(glob_match(probe, pattern) or glob_match(probe_file, pattern) for pattern in layer.paths):
            return layer.name
    return None


def _module_files(module: str) -> list[str]:
    if not module:
        return ["src/lib.rs", "src/main.rs"]
    path = module.replace("::", "/")
    return [
        f"src/{path}.rs",
        f"src/{path}/mod.rs",
    ]


def file_module_path(relative: str) -> str:
    posix = relative.replace("\\", "/")
    rest = posix[4:] if posix.startswith("src/") else posix
    if rest.endswith("/mod.rs"):
        rest = rest[: -len("/mod.rs")]
    elif rest.endswith(".rs"):
        rest = rest[:-3]
    if rest in {"lib", "main", ""}:
        return ""
    return rest.replace("/", "::")


def module_at_offset(masked: str, offset: int, file_module: str) -> str:
    prefix = masked[:offset]
    extra: list[str] = []
    depth_stack: list[str | None] = []
    i = 0
    while i < len(prefix):
        match = MOD_BLOCK.match(prefix, i)
        if match:
            extra_name = match.group(1)
            depth_stack.append(extra_name)
            extra.append(extra_name)
            i = match.end()
            continue
        ch = prefix[i]
        if ch == "{":
            depth_stack.append(None)
        elif ch == "}":
            if depth_stack:
                name = depth_stack.pop()
                if name is not None and extra:
                    extra.pop()
        i += 1
    parts = [part for part in [file_module, *extra] if part]
    return "::".join(parts)


def resolve_use_targets(
    tree: str,
    current_module: str,
    crate_name: str,
    crate_layers: dict[str, str],
) -> list[tuple[str, str | None]]:
    targets: list[tuple[str, str | None]] = []
    for raw in expand_braces("", collapse_ws(tree)):
        raw = AS_ALIAS.sub("", raw).strip()
        if not raw:
            continue
        resolved, hint = resolve_use_path(raw, current_module, crate_name, crate_layers)
        if resolved is None:
            continue
        targets.append((resolved, hint))
    return targets


def resolve_use_path(
    path: str,
    current_module: str,
    crate_name: str,
    crate_layers: dict[str, str],
) -> tuple[str | None, str | None]:
    parts = [part for part in path.split("::") if part]
    if not parts:
        return None, None
    if parts[-1] == "*":
        parts = parts[:-1]
    if not parts:
        return None, None

    i = 0
    module_parts: list[str]
    if parts[0] in {"crate", crate_name} and parts[0]:
        module_parts = []
        i = 1
    elif parts[0] == "self":
        module_parts = current_module.split("::") if current_module else []
        i = 1
    elif parts[0] == "super":
        module_parts = current_module.split("::") if current_module else []
        while i < len(parts) and parts[i] == "super":
            if module_parts:
                module_parts.pop()
            i += 1
    elif parts[0] in crate_layers:
        return parts[0], crate_layers[parts[0]]
    else:
        return None, None

    module_parts.extend(parts[i:])
    return "::".join(part for part in module_parts if part), None


def expand_braces(prefix: str, tree: str) -> list[str]:
    tree = tree.strip()
    if not tree:
        return [prefix] if prefix else []
    tree = AS_ALIAS.sub("", tree).strip()
    if tree.startswith("{"):
        if not tree.endswith("}"):
            raise ValueError("unbalanced use tree")
        results: list[str] = []
        for item in split_top_level(tree[1:-1]):
            results.extend(expand_braces(prefix, item))
        return results
    brace = tree.find("{")
    if brace != -1:
        head = tree[:brace].rstrip(":").rstrip()
        rest = tree[brace:]
        new_prefix = _join_path(prefix, head) if head else prefix
        return expand_braces(new_prefix, rest)
    return [_join_path(prefix, tree) if prefix else tree]


def split_top_level(text: str) -> list[str]:
    items: list[str] = []
    buf: list[str] = []
    depth = 0
    for ch in text:
        if ch == "{":
            depth += 1
            buf.append(ch)
        elif ch == "}":
            depth -= 1
            buf.append(ch)
        elif ch == "," and depth == 0:
            item = "".join(buf).strip()
            if item:
                items.append(item)
            buf = []
        else:
            buf.append(ch)
    item = "".join(buf).strip()
    if item:
        items.append(item)
    return items


def _join_path(prefix: str, part: str) -> str:
    part = part.strip().strip(":")
    if not prefix:
        return part
    if not part:
        return prefix
    return f"{prefix}::{part}"


def _find_semicolon(text: str, start: int) -> int | None:
    depth = 0
    i = start
    while i < len(text):
        ch = text[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        elif ch == ";" and depth == 0:
            return i
        i += 1
    return None


def mask_comments_and_strings(src: str) -> str:
    out: list[str] = []
    i = 0
    n = len(src)

    def masked(chunk: str) -> str:
        return "".join("\n" if ch == "\n" else " " for ch in chunk)

    while i < n:
        if src[i] in {"b", "c", "r"} and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            raw_at = i
            if src[i] in {"b", "c"}:
                raw_at = i + 1
            if raw_at < n and src[raw_at] == "r":
                j = raw_at + 1
                hashes = 0
                while j < n and src[j] == "#":
                    hashes += 1
                    j += 1
                if j < n and src[j] == '"':
                    closer = '"' + "#" * hashes
                    k = src.find(closer, j + 1)
                    if k == -1:
                        out.append(masked(src[i:]))
                        break
                    out.append(masked(src[i : k + len(closer)]))
                    i = k + len(closer)
                    continue
        if src.startswith("//", i):
            nl = src.find("\n", i)
            if nl == -1:
                out.append(masked(src[i:]))
                break
            out.append(masked(src[i:nl]))
            i = nl
            continue
        if src.startswith("/*", i):
            end = src.find("*/", i + 2)
            if end == -1:
                out.append(masked(src[i:]))
                break
            out.append(masked(src[i : end + 2]))
            i = end + 2
            continue
        if src[i] == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            out.append(masked(src[i:j]))
            i = j
            continue
        if src[i] == "'":
            if i + 1 < n and (src[i + 1].isalpha() or src[i + 1] == "_"):
                out.append(src[i])
                i += 1
                continue
            j = i + 1
            if j < n and src[j] == "\\":
                j += 2
            elif j < n:
                j += 1
            if j < n and src[j] == "'":
                j += 1
            out.append(masked(src[i:j]))
            i = j
            continue
        out.append(src[i])
        i += 1
    return "".join(out)


def blank_cfg_test_items(text: str) -> str:
    chars = list(text)
    i = 0
    while True:
        pos = text.find("#[cfg(test)]", i)
        if pos == -1:
            break
        j = pos + len("#[cfg(test)]")
        while True:
            j = _skip_ws(text, j)
            if text.startswith("#[", j):
                end = text.find("]", j)
                if end == -1:
                    j = len(text)
                    break
                j = end + 1
                continue
            break
        end = _skip_item(text, j)
        for index in range(pos, end):
            if chars[index] != "\n":
                chars[index] = " "
        i = max(end, pos + 1)
    return "".join(chars)


def _skip_ws(text: str, i: int) -> int:
    while i < len(text) and text[i] in " \t\r\n":
        i += 1
    return i


def _skip_item(text: str, i: int) -> int:
    depth = 0
    started = False
    while i < len(text):
        ch = text[i]
        if ch == "{":
            depth += 1
            started = True
        elif ch == "}":
            if depth:
                depth -= 1
            if started and depth == 0:
                return i + 1
        elif ch == ";" and depth == 0:
            return i + 1
        i += 1
    return i


def glob_match(path: str, pattern: str) -> bool:
    path = path.replace("\\", "/")
    pattern = pattern.replace("\\", "/")
    if path == pattern:
        return True
    if pattern.endswith("/**"):
        prefix = pattern[:-3]
        if path == prefix or path.startswith(prefix + "/"):
            return True
        if path == prefix + ".rs":
            return True
        return False
    regex = _glob_to_regex(pattern)
    return re.match(regex, path) is not None


def _glob_to_regex(pattern: str) -> str:
    parts = ["^"]
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


def collapse_ws(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def _snippet(original: str, line_no: int, fallback: str) -> str:
    lines = original.splitlines()
    if 1 <= line_no <= len(lines):
        return lines[line_no - 1].strip()
    return fallback


def _relative_posix(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def format_output(files: list[Path], edges: list[Edge], violations: list[Violation]) -> str:
    if not violations:
        return "\n".join(
            [
                "Architecture check passed",
                f"Files checked: {len(files)}",
                f"Dependency edges: {len(edges)}",
                "Violations: 0",
            ]
        )
    lines = ["Architecture check failed:", ""]
    for item in violations:
        lines.append(item.file.replace("\\", "/"))
        lines.append(f"  {item.title}")
        lines.append(f"  {item.detail}")
        if item.snippet:
            lines.append("")
            lines.append(f"  {item.snippet}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


if __name__ == "__main__":
    sys.exit(main())
