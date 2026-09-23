#!/usr/bin/env python3
"""Run one local check and leave a verifiable receipt beside its raw output."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import time


SCHEMA_VERSION = 1
ENV_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")


def utc_now():
    return datetime.now(timezone.utc).isoformat(timespec="microseconds")


def digest_file(path):
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
            size += len(block)
    return {"size": size, "sha256": digest.hexdigest()}


def file_state(path):
    try:
        before = path.lstat()
    except FileNotFoundError:
        return {"kind": "missing"}
    mode = stat.S_IMODE(before.st_mode)
    if stat.S_ISREG(before.st_mode):
        content = digest_file(path)
        after = path.lstat()
        if (before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
            after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns
        ):
            raise RuntimeError(f"file changed while hashing: {path}")
        return {"kind": "file", "mode": mode, **content}
    if stat.S_ISLNK(before.st_mode):
        target = os.readlink(path)
        raw = os.fsencode(target)
        return {"kind": "symlink", "target": target, "size": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest()}
    if stat.S_ISDIR(before.st_mode):
        return {"kind": "directory", "mode": mode}
    raise RuntimeError(f"unsupported file type in snapshot: {path}")


def git(root, *args):
    result = subprocess.run(["git", "-C", str(root), *args], stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, check=False)
    if result.returncode:
        message = result.stderr.decode("utf-8", "replace").strip()
        raise RuntimeError(f"git {' '.join(args)} failed: {message}")
    return result.stdout


def git_text(root, *args):
    return git(root, *args).decode("utf-8", "surrogateescape").removesuffix("\n")


def source_state(root):
    head = git_text(root, "rev-parse", "--verify", "HEAD")
    tree = git_text(root, "rev-parse", "--verify", "HEAD^{tree}")
    index = []
    tracked = set()
    for entry in git(root, "ls-files", "--cached", "--stage", "-z").split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        mode, oid, stage = metadata.decode("ascii").split()
        path = os.fsdecode(raw_path)
        if mode == "160000":
            raise RuntimeError(f"gitlink/submodule input unsupported: {path}")
        index.append({"path": path, "mode": mode, "oid": oid, "stage": int(stage)})
        tracked.add(path)
    untracked = {os.fsdecode(path) for path in
                 git(root, "ls-files", "--others", "--exclude-standard", "-z").split(b"\0")
                 if path}
    paths = sorted(tracked | untracked)
    working_files = {}
    for path in paths:
        state = file_state(root / path)
        if state["kind"] == "directory":
            raise RuntimeError(f"source directory input unsupported: {path}")
        working_files[path] = state
    return {"head": head, "committed_tree": tree, "index": index,
            "working_files": working_files,
            "untracked": sorted(untracked)}


def artifact_state(output):
    artifacts = {}
    for directory, dirs, files in os.walk(output, followlinks=False):
        for name in sorted(dirs + files):
            path = Path(directory) / name
            relative = path.relative_to(output).as_posix()
            if relative in {"receipt.json", ".receipt.tmp"}:
                continue
            artifacts[relative] = file_state(path)
    for required in ("stdout.bin", "stderr.bin"):
        if artifacts.get(required, {}).get("kind") != "file":
            raise RuntimeError(f"raw output is missing: {required}")
    return artifacts


def write_receipt(output, receipt):
    temporary = output / ".receipt.tmp"
    with temporary.open("x", encoding="utf-8") as stream:
        json.dump(receipt, stream, indent=2, sort_keys=True, ensure_ascii=True)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, output / "receipt.json")


def stop_process_group(process):
    # Linux: start_new_session makes the child's PID its private process group.
    if process.returncode is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        time.sleep(0.5)
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    process.wait(timeout=5)


def parse_args(arguments):
    if "--" not in arguments:
        raise ValueError("put -- before the literal command argv")
    separator = arguments.index("--")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--cwd", type=Path, default=Path.cwd())
    parser.add_argument("--timeout", type=float)
    parser.add_argument("--env", action="append", default=[], metavar="NAME=VALUE")
    options = parser.parse_args(arguments[:separator])
    command = arguments[separator + 1:]
    if not command:
        raise ValueError("command argv is empty")
    if options.timeout is not None and (not math.isfinite(options.timeout) or options.timeout <= 0):
        raise ValueError("--timeout must be a positive finite number")
    overrides = {}
    for item in options.env:
        name, delimiter, value = item.partition("=")
        if not delimiter or not ENV_NAME.fullmatch(name) or name in overrides:
            raise ValueError(f"invalid or repeated --env name: {name}")
        overrides[name] = value
    return options, command, overrides


def inside(path, root):
    return path == root or root in path.parents


def main(arguments):
    try:
        options, command, overrides = parse_args(arguments)
        cwd = options.cwd.resolve(strict=True)
        if not cwd.is_dir():
            raise ValueError("--cwd must be a directory")
        root = Path(git_text(cwd, "rev-parse", "--show-toplevel")).resolve(strict=True)
        if not inside(cwd, root):
            raise ValueError("Git checkout root does not contain --cwd")
        if not options.output.is_absolute():
            raise ValueError("--output must be an absolute path")
        output = options.output
        lexical = Path(os.path.abspath(output))
        resolved = output.resolve(strict=False)
        if inside(lexical, root) or inside(resolved, root):
            raise ValueError("--output must be outside the Git checkout")
        os.mkdir(output, 0o700)  # Exclusive creation: a prior receipt is never overwritten.
    except (OSError, ValueError, RuntimeError) as error:
        print(f"check_receipt: {error}", file=sys.stderr)
        return 2

    started = time.monotonic()
    receipt = {"schema_version": SCHEMA_VERSION, "status": "incomplete",
               "started_at": utc_now(), "ended_at": None, "duration_seconds": None,
               "argv": command, "cwd": str(cwd), "checkout": str(root),
               "env_overrides": overrides, "timeout_seconds": options.timeout,
               "child_pid": None, "process_group_id": None,
               "child_started_at": None, "child_ended_at": None,
               "child_duration_seconds": None, "child_returncode": None,
               "source_changed": None}

    def finish(status, code, error=None):
        receipt["status"] = status
        receipt["ended_at"] = utc_now()
        receipt["duration_seconds"] = round(time.monotonic() - started, 6)
        if error is not None:
            receipt["error"] = str(error)
            print(f"check_receipt: {error}", file=sys.stderr)
        write_receipt(output, receipt)
        return code

    write_receipt(output, receipt)
    try:
        receipt["inputs_before"] = source_state(root)
        write_receipt(output, receipt)
    except (OSError, RuntimeError, ValueError) as error:
        return finish("snapshot_error", 1, error)

    run_status = None
    try:
        with (output / "stdout.bin").open("xb") as stdout, (output / "stderr.bin").open("xb") as stderr:
            environment = os.environ.copy()
            environment.update(overrides)
            process = subprocess.Popen(command, cwd=cwd, env=environment,
                                       stdout=stdout, stderr=stderr, start_new_session=True)
            child_started = time.monotonic()
            receipt["child_started_at"] = utc_now()
            receipt["child_pid"] = process.pid
            receipt["process_group_id"] = process.pid
            try:
                write_receipt(output, receipt)
                try:
                    receipt["child_returncode"] = process.wait(timeout=options.timeout)
                except subprocess.TimeoutExpired:
                    stop_process_group(process)
                    receipt["child_returncode"] = process.returncode
                    run_status = "timeout"
                except KeyboardInterrupt:
                    stop_process_group(process)
                    receipt["child_returncode"] = process.returncode
                    run_status = "interrupted"
            except BaseException:
                if process.returncode is None:
                    stop_process_group(process)
                raise
            finally:
                receipt["child_ended_at"] = utc_now()
                receipt["child_duration_seconds"] = round(time.monotonic() - child_started, 6)
    except OSError as error:
        run_status = "spawn_error"
        receipt["error"] = str(error)

    try:
        receipt["inputs_after"] = source_state(root)
        receipt["source_changed"] = receipt["inputs_before"] != receipt["inputs_after"]
    except (OSError, RuntimeError, ValueError) as error:
        run_status = "snapshot_error"
        receipt["error"] = str(error)
    try:
        receipt["artifacts"] = artifact_state(output)
    except (OSError, RuntimeError, ValueError) as error:
        run_status = "artifact_error"
        receipt["error"] = str(error)

    if run_status:
        return finish(run_status, 130 if run_status == "interrupted" else
                      124 if run_status == "timeout" else 1)
    if receipt["child_returncode"] != 0:
        return finish("command_failed", 1)
    if receipt["source_changed"]:
        return finish("inputs_changed", 1)
    return finish("passed", 0)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
