"""Bounded, append-only evidence; no adapter or provider-specific payloads."""
import hashlib
import json
import os
from datetime import datetime, timezone
from pathlib import Path
import re


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(value):
    return hashlib.sha256(encoded(value)).hexdigest()


class Evidence:
    RESERVE = 131072

    def __init__(self, directory, limit, captures):
        self.directory = Path(directory)
        self.directory.mkdir(mode=0o700)  # Never replace an earlier run.
        self.limit, self.capture_limit = limit, captures
        self.used = self.sequence = self.captures = 0
        self.events = (self.directory / "events.jsonl").open("xb")

    def event(self, kind, data):
        row = encoded({"version": 1, "sequence": self.sequence,
                       "at": datetime.now(timezone.utc).isoformat(), "event": kind, "data": data}) + b"\n"
        if self.used + len(row) > self.limit - self.RESERVE:
            raise ValueError("evidence_budget")
        self.events.write(row)
        self.events.flush()
        os.fsync(self.events.fileno())
        self.used += len(row)
        self.sequence += 1

    def capture(self, name, data):
        if not re.fullmatch(r"[a-z0-9_-]+\.png", name) or self.captures >= self.capture_limit:
            raise ValueError("capture_budget_or_name")
        if self.used + len(data) > self.limit - self.RESERVE:
            raise ValueError("evidence_budget")
        with (self.directory / name).open("xb") as out:
            out.write(data)
        self.used += len(data)
        self.captures += 1

    def finish(self, report, replay):
        self.events.close()
        files = {"report.json": encoded(report), "replay.json": encoded(replay)}
        manifest = {p.name: {"bytes": p.stat().st_size,
                             "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
                    for p in self.directory.iterdir() if p.is_file()}
        manifest.update({n: {"bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()}
                         for n, b in files.items()})
        files["artifacts.json"] = encoded(manifest)
        if self.used + sum(map(len, files.values())) > self.limit:
            raise ValueError("final_evidence_budget")
        for name, data in files.items():
            with (self.directory / name).open("xb") as out:
                out.write(data)
        self.used += sum(map(len, files.values()))


def load_replay(path):
    """Bound file size before parsing, including files supplied outside our bundle."""
    with Path(path).open('rb') as source:
        data = source.read(65537)
    if len(data) > 65536:
        raise ValueError('replay_size')
    return json.loads(data)
