"""Synthetic fixtures only. No game assets, rules, locations or captures."""
import asyncio
import json
from pathlib import Path
import tempfile
import unittest

from redshirt import Candidate, Limits, Observation, Stop, Verdict, run
from redshirt.evidence import Evidence, digest
from redshirt.providers import MockTransport, Scripted, Seeded


class Fixture:
    identity = {"adapter": "synthetic-v1", "build": "fixture-v1", "reset": "empty"}

    def __init__(self, *, hidden=41, omit_input=False):
        self.hidden = hidden
        self.omit_input = omit_input
        self.closed = self.finalized = False
        self.value = self.expected = self.revision = 0
        self.ready = True
        self.terminal = None
        self.epoch = "one"

    async def reset(self):
        self.value = self.expected = self.revision = 0

    async def observe(self):
        return Observation("fixture", self.epoch, str(self.revision), {"counter": self.value},
                           ready=self.ready, terminal=self.terminal)

    def candidates(self, observation):
        return [Candidate("increment", "Press the increment control.", {"button": "increment"})]

    async def verify(self):
        pass

    async def execute(self, operation):
        self.expected += 1
        if not self.omit_input:
            self.value += 1
        self.revision += 1
        return {"input": operation, "acknowledged": True}  # Not the correctness verdict.

    async def evaluate(self, phase, operation):
        if phase == "final":
            self.finalized = True
        return Verdict(self.value == self.expected, phase == "after" and not self.omit_input,
                       {"expected": self.expected, "actual": self.value})

    async def close(self):
        self.closed = True


class RunnerTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.serial = 0

    def output(self):
        self.serial += 1
        return Path(self.tmp.name) / str(self.serial)

    async def checked_run(self, env, **kwargs):
        result = await run(env, self.output(), **kwargs)
        self.assertTrue(env.finalized)
        self.assertTrue(env.closed)
        return result

    async def test_concrete_replay_and_artifact_hashes(self):
        folder = self.output()
        first = await run(Fixture(), folder, provider=Scripted(["increment"] * 3))
        self.assertEqual(first["attempted_inputs"], 3)
        self.assertTrue(first["replayable"])
        saved = json.loads((folder / "replay.json").read_text())
        replay = await self.checked_run(Fixture(), replay=saved)
        self.assertTrue(replay["replay_complete"])
        self.assertEqual(replay["requests"], 0)
        self.assertEqual(first["evaluations"], replay["evaluations"])
        import hashlib
        for name, expected in json.loads((folder / "artifacts.json").read_text()).items():
            data = (folder / name).read_bytes()
            self.assertEqual(len(data), expected["bytes"])
            self.assertEqual(hashlib.sha256(data).hexdigest(), expected["sha256"])

    async def test_selector_stop_preserves_final_checks(self):
        report = await self.checked_run(Fixture(), provider=Scripted(["stop"]))
        self.assertEqual(report["stop"], "selector_stop")
        self.assertEqual(report["attempted_inputs"], 0)

    async def test_negative_control_detects_lying_receipt(self):
        report = await self.checked_run(Fixture(omit_input=True), provider=Scripted(["increment"]))
        self.assertEqual(report["stop"], "evaluation_failed")
        self.assertFalse(report["final"]["ok"])
        self.assertFalse(report["replayable"])

    async def test_busy_stale_epoch_and_changed_candidates_refuse(self):
        for mode, reason in [("busy", "busy_refused"), ("stale", "stale_observation"),
                             ("epoch", "stale_observation"), ("target", "candidate_changed")]:
            with self.subTest(mode=mode):
                env = Fixture()
                if mode == "busy":
                    env.ready = False

                async def transport(request):
                    if mode == "stale":
                        env.revision += 1
                    elif mode == "epoch":
                        env.epoch = "two"
                    elif mode == "target":
                        env.candidates = lambda obs: [Candidate("increment", "Replaced target", {"button": "other"})]
                    return b'{"candidate_id":"increment"}'

                report = await self.checked_run(env, provider=MockTransport(transport))
                self.assertEqual(report["stop"], reason)
                self.assertEqual(report["attempted_inputs"], 0)
                self.assertEqual(env.value, 0)

    async def test_hidden_state_does_not_enter_request(self):
        requests = []

        async def transport(request):
            requests.append(request)
            return b'{"candidate_id":"stop"}'

        for hidden in [17, {"secret": "unseen object", "random": 999}]:
            await self.checked_run(Fixture(hidden=hidden), provider=MockTransport(transport))
        self.assertEqual(requests[0], requests[1])
        self.assertEqual(set(requests[0]), {"version", "observation", "candidates", "remaining_inputs"})

    async def test_cancel_during_request_never_dispatches(self):
        cancel = asyncio.Event()
        started = asyncio.Event()

        async def transport(request):
            started.set()
            await asyncio.sleep(60)

        env = Fixture()
        task = asyncio.create_task(run(env, self.output(), provider=MockTransport(transport), cancel=cancel))
        await started.wait()
        cancel.set()
        report = await asyncio.wait_for(task, 1)
        self.assertEqual(report["stop"], "cancelled")
        self.assertEqual(report["attempted_inputs"], 0)
        self.assertTrue(env.finalized and env.closed)

    async def test_malformed_unknown_and_duplicate_mock_answers(self):
        for raw in [b'{"candidate_id":"increment","command":"anything"}',
                    b'{"candidate_id":"stop","candidate_id":"increment"}', b'null',
                    b'{"candidate_id":"absent"}', b'{"candidate_id":7}']:
            async def transport(request):
                return raw
            report = await self.checked_run(Fixture(), provider=MockTransport(transport))
            self.assertEqual(report["attempted_inputs"], 0)
            self.assertIn(report["stop"], ["harness_error", "unknown_candidate"])
            self.assertEqual(report["requests"], 1)

    async def test_request_input_time_and_no_progress_limits(self):
        report = await self.checked_run(Fixture(), provider=Scripted(["increment"] * 8), limits=Limits(inputs=2))
        self.assertEqual(report["attempted_inputs"], 2)
        self.assertEqual(report["stop"], "input_budget")
        report = await self.checked_run(Fixture(), provider=Scripted(["increment"] * 8), limits=Limits(requests=2))
        self.assertEqual(report["requests"], 2)
        self.assertEqual(report["stop"], "request_budget")

        async def slow(request):
            await asyncio.sleep(1)
            return b'{"candidate_id":"increment"}'

        report = await self.checked_run(Fixture(), provider=MockTransport(slow), limits=Limits(operation_seconds=.01))
        self.assertEqual(report["stop"], "operation_timeout")
        self.assertEqual(report["attempted_inputs"], 0)
        env = Fixture()
        original = env.evaluate
        async def stationary(phase, operation):
            check = await original(phase, operation)
            return Verdict(check.ok, False, check.checks)
        env.evaluate = stationary
        report = await self.checked_run(env, provider=Scripted(["increment"] * 8))
        self.assertEqual(report["stop"], "no_progress")
        self.assertEqual(report["attempted_inputs"], 3)

    async def test_death_unknown_and_environment_refusal(self):
        for reason in ["death", "unknown_state", "user_intervention"]:
            env = Fixture()
            env.terminal = reason
            report = await self.checked_run(env, provider=Scripted(["increment"]))
            self.assertEqual(report["stop"], reason)
            self.assertEqual(report["requests"], 0)
        env = Fixture()
        async def refuse():
            raise Stop("environment_refused")
        env.verify = refuse
        report = await self.checked_run(env, provider=Scripted(["increment"]))
        self.assertEqual(report["stop"], "environment_refused")
        self.assertEqual(env.value, 0)

    async def test_replay_identity_and_preconditions_are_enforced(self):
        for replay in [{"version": 1, "identity": {}, "steps": [], "complete": True},
                       {"version": 1, "identity": Fixture.identity, "steps": [], "complete": False},
                       {"version": 1, "identity": Fixture.identity, "complete": True,
                        "steps": [{"operation": {"button": "increment"}, "view": "stale", "verdict": {}}]}]:
            report = await self.checked_run(Fixture(), replay=replay)
            self.assertFalse(report["replay_complete"])
            self.assertEqual(report["attempted_inputs"], 0)

    async def test_provider_never_controls_mutable_request_or_candidate(self):
        env = Fixture()
        async def transport(request):
            request["observation"]["counter"] = 900
            request["candidates"]["increment"] = "Execute something else"
            return b'{"candidate_id":"increment"}'
        report = await self.checked_run(env, provider=MockTransport(transport), limits=Limits(inputs=1))
        self.assertEqual(env.value, 1)
        self.assertEqual(report["operations"], [{"button": "increment"}])

    async def test_mock_transport_single_flight(self):
        gate = asyncio.Event()
        async def transport(request):
            await gate.wait()
            return b'{"candidate_id":"stop"}'
        provider = MockTransport(transport)
        task = asyncio.create_task(provider.select({}))
        await asyncio.sleep(0)
        with self.assertRaisesRegex(ValueError, "provider_busy"):
            await provider.select({})
        gate.set()
        self.assertEqual(await task, "stop")

    def test_evidence_does_not_overwrite_or_escape_and_reserves_finalization(self):
        folder = self.output()
        evidence = Evidence(folder, 262144, 1)
        evidence.capture("one.png", b"synthetic bytes, not a screenshot")
        with self.assertRaises(ValueError):
            evidence.capture("../escape.png", b"x")
        with self.assertRaises(ValueError):
            evidence.event("oversize", "x" * 262144)
        evidence.finish({"ok": True}, {"steps": []})
        with self.assertRaises(FileExistsError):
            Evidence(folder, 262144, 1)

    async def test_uncertain_input_is_retained_but_refuses_replay(self):
        env = Fixture()
        async def lost_receipt(operation):
            env.expected += 1
            env.value += 1
            raise OSError('receipt lost after input')
        env.execute = lost_receipt
        folder = self.output()
        report = await run(env, folder, provider=Scripted(['increment']))
        self.assertEqual(report['attempted_inputs'], 1)
        self.assertEqual(report['operations'], [{'button': 'increment'}])
        self.assertFalse(report['replayable'])
        saved = json.loads((folder / 'replay.json').read_text())
        repeated = await self.checked_run(Fixture(), replay=saved)
        self.assertEqual(repeated['stop'], 'replay_identity_or_shape')
        self.assertFalse(repeated['replay_complete'])

    async def test_identity_changed_while_provider_waits_refuses(self):
        env = Fixture()
        env.identity = dict(Fixture.identity)
        async def transport(request):
            env.identity['build'] = 'other'
            return b'{"candidate_id":"increment"}'
        report = await self.checked_run(env, provider=MockTransport(transport))
        self.assertEqual(report['stop'], 'identity_changed')
        self.assertEqual(report['attempted_inputs'], 0)

    async def test_seeded_selector_is_reproducible(self):
        request = {"candidates": {"a": "A", "b": "B", "stop": "Stop"}}
        first, second = Seeded(7), Seeded(7)
        self.assertEqual([await first.select(request) for _ in range(10)],
                         [await second.select(request) for _ in range(10)])


if __name__ == "__main__":
    unittest.main()
