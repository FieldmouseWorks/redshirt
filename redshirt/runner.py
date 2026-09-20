"""One external controller. Adapters own observations, inputs, reset and checks.

The adapter is trusted code, not a sandbox. Its observation must be audited for
the intended audience; only view/description fields reach a decision provider.
"""
import asyncio
from dataclasses import asdict, dataclass
import json
import time
from typing import Protocol

from .evidence import Evidence, digest, encoded


class Stop(Exception):
    """A classified refusal with a code supplied by trusted adapter/controller code."""


@dataclass(frozen=True)
class Limits:
    inputs: int = 24
    seconds: float = 180
    operation_seconds: float = 10
    final_seconds: float = 10
    evidence_bytes: int = 8 * 1024 * 1024
    captures: int = 12
    requests: int = 24
    no_progress: int = 3

    def validate(self):
        if not (1 <= self.inputs <= 24 and 0 < self.seconds <= 180
                and 0 < self.operation_seconds <= 10 and 0 < self.final_seconds <= 10
                and self.seconds > 2 * self.final_seconds
                and 262144 <= self.evidence_bytes <= 8 * 1024 * 1024
                and 0 <= self.captures <= 12 and 1 <= self.requests <= 24
                and 1 <= self.no_progress <= 3):
            raise ValueError("invalid_limits")


@dataclass(frozen=True)
class Observation:
    # Opaque guards never sent to providers; reset/revision/target are adapter-owned.
    environment: str
    epoch: str
    guard: str
    view: dict
    ready: bool = True
    terminal: str | None = None


@dataclass(frozen=True)
class Candidate:
    id: str
    description: str
    operation: dict
    inputs: int = 1
    needs_ready: bool = True


@dataclass(frozen=True)
class Verdict:
    ok: bool
    progress: bool
    checks: dict


class Adapter(Protocol):
    identity: dict  # Build, scenario, timing/setup and adapter contract identities.
    setup_mode: str  # Optional at runtime: 'reset' (default) or 'attach'.

    # Initialization hook. An attach adapter verifies an existing session here;
    # it must not claim a reset merely because the starting observation matches.
    async def reset(self) -> None: ...
    async def observe(self) -> Observation: ...
    def candidates(self, observation: Observation) -> list[Candidate]: ...
    async def verify(self) -> None: ...
    async def execute(self, operation: dict) -> dict: ...
    async def evaluate(self, phase: str, operation: dict | None) -> Verdict: ...
    async def close(self) -> None: ...


def candidates_for(adapter, observation):
    candidates = adapter.candidates(observation)
    ids = [c.id for c in candidates]
    if (len(candidates) > 16 or len(set(ids)) != len(ids) or "stop" in ids
            or any(not isinstance(c.id, str) or not c.id or len(c.id) > 80
                   or not isinstance(c.description, str) or len(c.description) > 512
                   or not 1 <= c.inputs <= 24 or len(encoded(c.operation)) > 512 for c in candidates)):
        raise Stop("invalid_candidates")
    if len(encoded([asdict(c) for c in candidates])) > 16384:
        raise Stop("candidate_size")
    return candidates


def verdict_data(check):
    value = asdict(check)
    if not isinstance(check.ok, bool) or not isinstance(check.progress, bool) or len(encoded(value)) > 1024:
        raise Stop('invalid_evaluation')
    return value


async def run(adapter: Adapter, output, *, provider=None, replay=None,
              limits=Limits(), cancel=None):
    """Run once; concrete replay never constructs or calls a provider.

    Final checks and close have separate reserved time and survive every stop.
    Replay refuses unavailable concrete candidates rather than adapting a trace.
    """
    limits.validate()
    if (provider is None) == (replay is None):
        raise ValueError("provide_exactly_one_selector_or_replay")
    cancel = cancel or asyncio.Event()
    identity = json.loads(encoded(adapter.identity))
    setup_mode = getattr(adapter, 'setup_mode', 'reset')
    if setup_mode not in ('reset', 'attach'):
        raise ValueError('invalid_setup_mode')
    if len(encoded(identity)) > 4096:
        raise ValueError('identity_size')
    evidence = Evidence(output, limits.evidence_bytes, limits.captures)
    started = time.monotonic()
    deadline = started + limits.seconds - 2 * limits.final_seconds
    report = {"version": 1, "identity": identity, "limits": asdict(limits),
              "provider": "none-replay" if replay is not None else provider.name,
              "requests": 0, "attempted_inputs": 0, "operations": [], "evaluations": [],
              "stop": "not_started", "setup_mode": setup_mode, "setup_verified": False,
              "reset_verified": False, "final": None, "cleanup": False}
    records = []
    baseline = None

    async def capture():
        method = getattr(adapter, "capture", None)
        if method and evidence.captures < limits.captures:
            evidence.capture(f"frame-{evidence.captures:02d}.png", await bounded(method()))

    def admit(cost=0):
        if cancel.is_set():
            raise Stop("cancelled")
        if time.monotonic() >= deadline:
            raise Stop("time_budget")
        if report["attempted_inputs"] + cost > limits.inputs:
            raise Stop("input_budget")

    async def bounded(call):
        remaining = min(limits.operation_seconds, deadline - time.monotonic())
        if remaining <= 0 or cancel.is_set():
            call.close()
            raise Stop('cancelled' if cancel.is_set() else 'time_budget')
        task = asyncio.create_task(call)
        cancelled = asyncio.create_task(cancel.wait())
        try:
            done, _ = await asyncio.wait([task, cancelled], timeout=max(0, remaining),
                                         return_when=asyncio.FIRST_COMPLETED)
            if cancelled in done:
                raise Stop("cancelled")
            if task not in done:
                raise Stop("operation_timeout")
            return task.result()
        finally:
            for pending in (task, cancelled):
                if not pending.done():
                    pending.cancel()
            await asyncio.gather(task, cancelled, return_exceptions=True)

    try:
        if replay is not None:
            if setup_mode == 'attach':
                raise Stop('replay_unavailable')
            if (not isinstance(replay, dict) or len(encoded(replay)) > 65536
                    or set(replay) != {"version", "identity", "steps", "complete"}
                    or replay["version"] != 1 or replay["identity"] != identity or replay['complete'] is not True
                    or not isinstance(replay["steps"], list) or len(replay["steps"]) > limits.inputs):
                raise Stop("replay_identity_or_shape")
        evidence.event("started", {"identity": identity, "limits": asdict(limits), "setup_mode": setup_mode})
        admit()
        await bounded(adapter.reset())
        check = await bounded(adapter.evaluate("reset", None))
        evidence.event("reset_checks", verdict_data(check))
        if not check.ok:
            raise Stop("reset_unverified")
        report["setup_verified"] = True
        report["reset_verified"] = setup_mode == 'reset'
        baseline = await bounded(adapter.observe())
        await capture()
        no_progress = 0
        while True:
            admit()
            observation = await bounded(adapter.observe())
            if len(encoded(observation.view)) > 4096:
                raise Stop('observation_size')
            # Round-trip to detach mutable dictionaries from adapter/provider references.
            snapshot = json.loads(encoded(asdict(observation)))
            if observation.environment != baseline.environment or observation.epoch != baseline.epoch:
                raise Stop("environment_changed")
            if observation.terminal:
                raise Stop(observation.terminal)
            candidates = candidates_for(adapter, observation)
            frozen = encoded([asdict(c) for c in candidates])
            if replay is not None:
                if len(records) == len(replay["steps"]):
                    report["stop"] = "replay_complete"
                    break
                saved = replay["steps"][len(records)]
                if set(saved) != {"operation", "view", "verdict"} or saved["view"] != digest(observation.view):
                    raise Stop("replay_precondition")
                matches = [c for c in candidates if c.operation == saved["operation"]]
                if len(matches) != 1:
                    raise Stop("replay_candidate_unavailable")
                chosen = matches[0].id
            else:
                if report["requests"] >= limits.requests:
                    raise Stop("request_budget")
                request = {"version": 1, "observation": observation.view,
                           "candidates": {c.id: c.description for c in candidates},
                           "remaining_inputs": limits.inputs - report["attempted_inputs"]}
                request["candidates"]["stop"] = "Stop the experiment."
                if len(encoded(request)) > 16384:
                    raise Stop("observation_size")
                evidence.event("request", request)
                report["requests"] += 1
                chosen = await bounded(provider.select(json.loads(encoded(request))))
            admit()
            if not isinstance(chosen, str) or len(chosen) > 80:
                raise Stop("invalid_decision")
            evidence.event("decision", {"candidate_id": chosen})
            if chosen == "stop":
                report["stop"] = "selector_stop"
                break
            candidate = next((c for c in candidates if c.id == chosen), None)
            if candidate is None:
                raise Stop("unknown_candidate")
            await bounded(adapter.verify())
            if adapter.identity != identity or getattr(adapter, 'setup_mode', 'reset') != setup_mode:
                raise Stop('identity_changed')
            current = await bounded(adapter.observe())
            if snapshot != json.loads(encoded(asdict(current))):
                raise Stop("stale_observation")
            if frozen != encoded([asdict(c) for c in candidates_for(adapter, current)]):
                raise Stop("candidate_changed")
            if candidate.needs_ready and not current.ready:
                raise Stop("busy_refused")
            admit(candidate.inputs)
            # Intent and attempted-input accounting precede the uncertain side effect.
            operation = json.loads(encoded(candidate.operation))
            evidence.event("intent", {"operation": operation, "view": observation.view})
            admit(candidate.inputs)
            report["attempted_inputs"] += candidate.inputs
            report["operations"].append(operation)
            receipt = await bounded(adapter.execute(operation))
            if len(encoded(receipt)) > 4096:
                raise Stop('receipt_size')
            evidence.event("receipt", receipt)
            check = await bounded(adapter.evaluate("after", operation))
            checked = verdict_data(check)
            evidence.event("checked", checked)
            await capture()
            report["evaluations"].append(checked)
            records.append({"operation": operation, "view": digest(observation.view), "verdict": checked})
            if replay is not None and saved["verdict"] != checked:
                raise Stop("replay_mismatch")
            if not check.ok:
                raise Stop("evaluation_failed")
            no_progress = 0 if check.progress else no_progress + 1
            if no_progress >= limits.no_progress:
                raise Stop("no_progress")
    except Stop as exc:
        report["stop"] = str(exc)
    except asyncio.CancelledError:
        report["stop"] = "cancelled"
    except Exception as exc:
        # Arbitrary exception messages may contain credentials or unrestricted state.
        report["stop"] = "harness_error"
        report["error_type"] = type(exc).__name__
    finally:
        try:
            check = await asyncio.wait_for(adapter.evaluate("final", None), limits.final_seconds)
            report["final"] = verdict_data(check)
        except Exception as exc:
            report["final"] = {"ok": False, "error_type": type(exc).__name__}
        try:
            await asyncio.wait_for(adapter.close(), limits.final_seconds)
            report["cleanup"] = True
        except Exception as exc:
            report["cleanup_error"] = type(exc).__name__
        report["elapsed_seconds"] = round(time.monotonic() - started, 6)
        report["replay_complete"] = (replay is not None and report["stop"] == "replay_complete"
                                     and report["final"]["ok"] and report["cleanup"])
        # An uncertain input must never disappear from a supposedly complete replay.
        report["replayable"] = (report["reset_verified"] and len(records) == len(report["operations"])
                                and getattr(adapter, 'setup_mode', 'reset') == setup_mode
                                and report["final"]["ok"] and report["cleanup"])
        evidence.finish(report, {"version": 1, "identity": identity,
                                 "complete": report['replayable'], "steps": records})
    return report
