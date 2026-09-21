"""Host-configured menu bounds with the retained Python controller and provider."""
from dataclasses import asdict
import json
from pathlib import Path
import tempfile
import unittest

from redshirt import Limits, run
from redshirt.evidence import encoded
from redshirt.jev import Jev, MAX_BYTES, MODEL
from redshirt.providers import Scripted
from wide_fixture import WideFixture, WIDE, mock


class WideMenuTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name); self.serial = 0

    async def episode(self, env, *, limits=None, provider=None, replay=None):
        self.serial += 1; folder = self.root / str(self.serial)
        result = await run(env, folder, limits=limits or Limits(captures=0, **WIDE),
                           provider=provider, replay=replay)
        self.assertTrue(env.closed and env.finalized and result['cleanup'])
        self.assertTrue(result['final']['ok'])
        return result, folder

    async def test_defaults_preserve_96_and_host_can_admit_254(self):
        for count, limits, reason, inputs in [(96, Limits(captures=0), 'selector_stop', 1),
                                              (97, Limits(captures=0), 'invalid_candidates', 0),
                                              (254, Limits(captures=0, **WIDE), 'selector_stop', 1),
                                              (255, Limits(captures=0, **WIDE), 'invalid_candidates', 0)]:
            env = WideFixture(count, 1)
            report, folder = await self.episode(env, limits=limits, provider=Scripted([env.choice(count-1), 'stop']))
            self.assertEqual((report['stop'], report['attempted_inputs']), (reason, inputs))
            if inputs:
                saved = json.loads((folder / 'replay.json').read_text())
                replay, _ = await self.episode(WideFixture(count, 1), limits=limits, replay=saved)
                self.assertTrue(replay['replay_complete']); self.assertEqual(replay['requests'], 0)

    async def test_exact_candidate_table_and_decision_byte_boundaries(self):
        env = WideFixture(8, 200); obs = await env.observe(); cs = env.candidates(obs)
        table_size = len(encoded([asdict(c) for c in cs]))
        request = {'version': 1, 'observation': obs.view,
                   'candidates': {**{c.id: c.description for c in cs}, 'stop': 'Stop the experiment.'}, 'remaining_inputs': 24}
        decision_size = len(encoded(request))
        for field, size, reason in [('candidate_bytes', table_size, 'candidate_size'), ('decision_bytes', decision_size, 'observation_size')]:
            for allowed in (size, size-1):
                report, _ = await self.episode(WideFixture(8, 200), limits=Limits(captures=0, **{**WIDE, field: allowed}), provider=Scripted(['stop']))
                self.assertEqual(report['stop'], 'selector_stop' if allowed == size else reason)
                self.assertEqual(report['requests'], int(allowed == size)); self.assertEqual(report['attempted_inputs'], 0)

    async def test_pre_input_rechecks_still_bind_the_whole_menu(self):
        for grow, limit, expected in [(True, 3, 'invalid_candidates'), (False, 3, 'candidate_changed')]:
            env = WideFixture(3, 1)
            async def verify():
                if grow: env.count += 1
                else: env.width += 1
            env.verify = verify
            result, _ = await self.episode(env, limits=Limits(captures=0, candidates=limit), provider=Scripted([env.choice(0)]))
            self.assertEqual(result['stop'], expected); self.assertEqual(result['attempted_inputs'], 0)

    async def test_wide_mock_preserves_every_choice_and_replays_without_a_provider(self):
        env = WideFixture(); provider = mock(env)
        result, folder = await self.episode(env, provider=provider)
        self.assertEqual((result['stop'], result['attempted_inputs'], result['requests']), ('selector_stop', 1, 2))
        events = [json.loads(x) for x in (folder / 'events.jsonl').read_text().splitlines()]
        receipts = [r['data'] for r in events if r['event'] == 'provider_receipt']
        self.assertEqual(len(receipts), 2)
        for row in receipts:
            body = row['request']; self.assertEqual(body['questions']['action']['criteria'], body['state']['candidates'])
            self.assertEqual(len(body['state']['candidates']), 201)
            self.assertGreater(len(encoded(body)), MAX_BYTES)
            self.assertLessEqual(len(encoded(row)), provider.evidence_limit)
        replay, _ = await self.episode(WideFixture(), replay=json.loads((folder / 'replay.json').read_text()))
        self.assertTrue(replay['replay_complete']); self.assertEqual(replay['requests'], 0)

    async def test_provider_request_boundary_and_unchanged_response_bound(self):
        env = WideFixture(); obs = await env.observe(); cs = env.candidates(obs)
        request = {'version': 1, 'observation': obs.view,
                   'candidates': {**{c.id: c.description for c in cs}, 'stop': 'Stop the experiment.'}, 'remaining_inputs': 24}
        provider = mock(env); await provider.select(request)
        size = len(encoded(provider.take_evidence()[0]['request']))
        for bound in (size, size-1, MAX_BYTES):
            provider = mock(env, request_bytes=bound)
            if bound == size: self.assertEqual(await provider.select(request), env.choice(199))
            else:
                with self.assertRaisesRegex(ValueError, 'request_size'): await provider.select(request)
            self.assertEqual(provider.calls, int(bound == size))
        async def oversized(_): return 200, b'x' * (MAX_BYTES+1)
        provider = Jev(oversized, request_bytes=65536)
        with self.assertRaisesRegex(ValueError, 'invalid_transport_response'): await provider.select(request)
        self.assertEqual(provider.calls, 1); self.assertEqual(len(provider.take_evidence()), 1)

    def test_new_host_limits_reject_wrong_types_and_excesses(self):
        self.assertEqual((Limits().candidates, Limits().candidate_bytes, Limits().decision_bytes), (96, 16384, 16384))
        for field, bad in [('candidates', [0, 255, True, 2.0, '2']), ('candidate_bytes', [1023, 65537, True, 2048.0]), ('decision_bytes', [1023, 32769, True, 2048.0])]:
            for value in bad:
                with self.subTest(field=field,value=value), self.assertRaises(ValueError): Limits(**{field:value}).validate()
        for value in (1023, 65537, True, 2048.0):
            with self.assertRaises(ValueError): Jev(None, request_bytes=value)
