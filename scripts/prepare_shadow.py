#!/usr/bin/env python3
"""Prepare a frozen, source-backed Jev shadow-ranking input; never dispatch a provider."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any

SOURCE_REVISION = "64d6348ddce3bfb10031155fe5de3957d09d38d2"
CASE_FILES = (
    ("terminal-replay", "terminal-replay.json"),
    ("packet-provenance", "packet-provenance.json"),
    ("duplicate-json-keys", "duplicate-json-keys.json"),
    ("jev-cancellation", "jev-cancellation.json"),
)
LIMITS = {"max_calls": 4, "max_reserved_usd": 0.02}
ROOT = Path(__file__).resolve().parents[1]
SPECS = ROOT / "examples" / "shadow"
KEY_TEMPLATE = SPECS / "essential-evidence-key.template.json"


class PreparationError(Exception):
    """A bounded local preparation step failed."""


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise PreparationError(f"cannot read JSON input {path}: {exc}") from exc


def write_new(path: Path, value: Any) -> None:
    data = (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")
    try:
        with path.open("xb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    except OSError as exc:
        raise PreparationError(f"cannot create {path}: {exc}") from exc


def write_bytes_new(path: Path, data: bytes) -> None:
    try:
        with path.open("xb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    except OSError as exc:
        raise PreparationError(f"cannot create {path}: {exc}") from exc


def parse_json_bytes(data: bytes, description: str) -> Any:
    try:
        return json.loads(data.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise PreparationError(f"invalid JSON from {description}: {exc}") from exc


def run_child(argv: list[str], *, timeout: float = 120.0) -> bytes:
    child_env = os.environ.copy()
    child_env.pop("TYPESAFE_API_KEY", None)
    try:
        result = subprocess.run(
            argv,
            env=child_env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise PreparationError(f"child command failed to run: {argv[0]}: {exc}") from exc
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise PreparationError(
            f"child exited {result.returncode}: {argv[0]}: {detail or '<no stderr>'}"
        )
    return result.stdout


def absolute_file(path: Path, description: str, executable: bool = False) -> Path:
    try:
        resolved = path.resolve(strict=True)
    except OSError as exc:
        raise PreparationError(f"{description} does not exist: {path}") from exc
    if not resolved.is_file():
        raise PreparationError(f"{description} must be a file: {resolved}")
    if executable and not os.access(resolved, os.X_OK):
        raise PreparationError(f"{description} is not executable: {resolved}")
    return resolved


def load_inputs() -> tuple[list[tuple[str, Path, dict[str, Any]]], dict[str, Any]]:
    inputs = []
    for case_id, filename in CASE_FILES:
        spec_path = SPECS / filename
        spec = read_json(spec_path)
        if spec.get("version") != 1 or spec.get("source_revision") != SOURCE_REVISION:
            raise PreparationError(f"unexpected packet spec version or source revision: {spec_path}")
        instructions = spec.get("required")
        if not isinstance(instructions, list) or not any(
            item.get("path") == "AGENTS.md"
            and item.get("start_line") == 1
            and item.get("end_line") == 90
            for item in instructions
            if isinstance(item, dict)
        ):
            raise PreparationError(f"spec must include the complete AGENTS.md: {spec_path}")
        chunks = spec.get("chunks")
        if not isinstance(chunks, list) or not 6 <= len(chunks) <= 8:
            raise PreparationError(f"spec must contain 6 to 8 candidate chunks: {spec_path}")
        inputs.append((case_id, spec_path, spec))

    key = read_json(KEY_TEMPLATE)
    ids = {case_id for case_id, _, _ in inputs}
    if (
        key.get("version") != 1
        or key.get("source_revision") != SOURCE_REVISION
        or key.get("manifest_sha256") is not None
        or set(key.get("cases", {})) != ids
    ):
        raise PreparationError(f"essential-evidence key template does not match fixture set: {KEY_TEMPLATE}")
    for case_id, _, spec in inputs:
        case_key = key["cases"][case_id]
        chunk_ids = {chunk.get("id") for chunk in spec["chunks"] if isinstance(chunk, dict)}
        essential = case_key.get("essential")
        rationale = case_key.get("rationale")
        if (
            not isinstance(essential, list)
            or not essential
            or not all(isinstance(item, str) for item in essential)
            or len(set(essential)) != len(essential)
            or not set(essential) <= chunk_ids
            or not isinstance(case_key.get("expected_finding"), str)
            or not isinstance(case_key.get("proof"), list)
            or not isinstance(rationale, dict)
            or not set(essential) <= set(rationale) <= chunk_ids
        ):
            raise PreparationError(f"key labels must identify declared chunks and matching rationales: {case_id}")
    return inputs, key


def prepare(args: argparse.Namespace) -> dict[str, Any]:
    packet_binary = absolute_file(args.packet_binary, "packet binary", executable=True)
    shadow_binary = absolute_file(args.shadow_binary, "shadow binary", executable=True)
    try:
        source_repo = args.source_repo.resolve(strict=True)
    except OSError as exc:
        raise PreparationError(f"source repo does not exist: {args.source_repo}") from exc
    if not source_repo.is_dir():
        raise PreparationError(f"source repo is not a directory: {source_repo}")
    output = args.output.absolute()
    if os.path.lexists(output):
        raise PreparationError(f"output already exists; choose a fresh path: {output}")
    if not output.parent.is_dir():
        raise PreparationError(f"output parent must already exist: {output.parent}")

    try:
        revision = run_child(["git", "-C", str(source_repo), "rev-parse", "HEAD"]).decode(
            "ascii", errors="strict"
        ).strip()
    except UnicodeError as exc:
        raise PreparationError("git returned a non-ASCII HEAD") from exc
    if revision != SOURCE_REVISION:
        raise PreparationError(
            f"source repo must be at {SOURCE_REVISION}; observed {revision}"
        )

    inputs, key_template = load_inputs()
    try:
        output.mkdir(mode=0o700)
        packets_dir = output / "packets"
        packets_dir.mkdir(mode=0o700)
    except OSError as exc:
        raise PreparationError(f"cannot create fresh output directory {output}: {exc}") from exc
    cases = []
    build_summaries = {}
    for case_id, spec_path, _ in inputs:
        packet_path = packets_dir / f"{case_id}.packet.json"
        build_stdout = run_child(
            [
                str(packet_binary),
                "build",
                "--repo",
                str(source_repo),
                "--spec",
                str(spec_path),
                "--output",
                str(packet_path),
            ]
        )
        build_summary = parse_json_bytes(build_stdout, f"packet build for {case_id}")
        if not isinstance(build_summary, dict):
            raise PreparationError(f"packet build returned a non-object summary: {case_id}")
        if build_summary.get("freshness") != "not_checked":
            raise PreparationError(f"packet build claimed unexpected freshness: {case_id}")
        packet = read_json(packet_path)
        if packet.get("source_revision") != SOURCE_REVISION:
            raise PreparationError(f"built packet has wrong source revision: {case_id}")
        cases.append({"id": case_id, "packet": packet})
        build_summaries[case_id] = build_summary

    manifest = {
        "version": 1,
        "limits": LIMITS,
        "cases": cases,
    }
    manifest_path = output / "shadow-manifest.json"
    write_new(manifest_path, manifest)
    preflight_stdout = run_child(
        [
            str(shadow_binary),
            "--manifest",
            str(manifest_path),
            "--repo",
            str(source_repo),
            "--preflight",
        ]
    )
    preflight = parse_json_bytes(preflight_stdout, "shadow preflight")
    if not isinstance(preflight, dict):
        raise PreparationError("shadow preflight returned a non-object summary")
    digest = preflight.get("manifest_sha256")
    if (
        not isinstance(digest, str)
        or len(digest) != 64
        or any(character not in "0123456789abcdef" for character in digest)
    ):
        raise PreparationError("shadow preflight did not return a manifest_sha256")
    if preflight.get("planned_calls") != len(CASE_FILES):
        raise PreparationError("shadow preflight planned an unexpected number of calls")

    bound_key = dict(key_template)
    bound_key["manifest_sha256"] = digest
    write_new(output / "essential-evidence-key.json", bound_key)
    write_new(output / "packet-build-summaries.json", build_summaries)
    write_bytes_new(output / "preflight.json", preflight_stdout)
    summary = {
        "output": str(output),
        "source_revision": SOURCE_REVISION,
        "manifest_sha256": digest,
        "packet_count": len(cases),
        "planned_calls": preflight["planned_calls"],
        "reserved_usd": preflight.get("reserved_usd"),
        "preflight": str(output / "preflight.json"),
        "manifest": str(manifest_path),
        "key": str(output / "essential-evidence-key.json"),
    }
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packet-binary", type=Path, required=True)
    parser.add_argument("--shadow-binary", type=Path, required=True)
    parser.add_argument("--source-repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="new output directory")
    args = parser.parse_args()
    try:
        result = prepare(args)
    except (PreparationError, json.JSONDecodeError) as exc:
        print(f"prepare_shadow.py: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
