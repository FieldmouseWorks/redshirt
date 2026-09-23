"""Unchanged Python client against the Rust owner, over actual cancellable pipes."""
import asyncio
from dataclasses import asdict
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import time
import unittest

from redshirt import Limits, Observation, run
from redshirt.interaction import InteractionClient, choice_tool, completion
from test_runner import Fixture

BINARY = os.environ.get('REDSHIRT_BIN')
WORKER = Path(__file__).with_name('rust_fixture.py')


@unittest.skipUnless(BINARY, 'set REDSHIRT_BIN to the built Rust executable')
class RustInteractionTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.serial = 0

    async def start(self, mode='normal', *, short=False):
        self.serial += 1
        folder = self.root / str(self.serial)
        argv = [BINARY, '--output', str(folder), '--stdio', '--limits-json',
                json.dumps(asdict(Limits(captures=0, operation_seconds=.3 if short else 10))),
                '--adapter', sys.executable, str(WORKER), mode]
        client = await InteractionClient.start(*argv)
        self.addAsyncCleanup(client.close)
        return client, folder

    async def report(self, client, folder):
        # stdin deliberately remains open; shutdown must not wait for a read thread.
        await asyncio.wait_for(client.process.wait(), 5)
        report = json.loads((folder / 'report.json').read_text())
        self.assertTrue(report['cleanup'])
        self.assertEqual(report['controller'], 'rust-v1')
        return report

    def receipts(self, folder):
        return [r['data'] for r in map(json.loads, (folder / 'events.jsonl').read_text().splitlines())
                if r['event'] == 'provider_receipt']

    async def test_exact_wire_parity_and_model_free_replay(self):
        rust, folder = await self.start()
        baseline = await InteractionClient.start(sys.executable,
            str(Path(__file__).with_name('interactive_fixture.py')), str(self.root / 'python'))
        self.addAsyncCleanup(baseline.close)
        tokens = []
        for action in ['increment', 'increment', 'stop']:
            a, b = rust.observe(), baseline.observe()
            tokens.append(a.pop('decision_id'))
            b.pop('decision_id')
            self.assertEqual(a, b)
            self.assertEqual(a['tool'], choice_tool(a['candidates']))
            await rust.act(action)
            await baseline.act(action)
        self.assertEqual(len(set(tokens)), 3)
        self.assertTrue(all(len(t) == 32 and int(t, 16) >= 0 for t in tokens))
        report = await self.report(rust, folder)
        self.assertEqual(rust.observe(), completion(report))
        self.assertEqual(rust.observe(), baseline.observe())
        saved = json.loads((folder / 'replay.json').read_text())
        replay = await run(Fixture(), self.root / 'replay', replay=saved, limits=Limits(captures=0))
        self.assertTrue(replay['replay_complete'])
        self.assertEqual(replay['requests'], 0)
        self.assertEqual(len(self.receipts(folder)), 3)

    async def test_completed_sessions_reap_with_exact_exit_status(self):
        for _ in range(4):
            client, folder = await self.start()
            pid = client.process.pid
            children_path = Path(f'/proc/{pid}/task/{pid}/children')
            adapter_pids = [int(value) for value in children_path.read_text().split()] if children_path.exists() else []
            await client.act('increment')
            done = await client.act('stop')
            self.assertEqual(done['stop'], 'selector_stop')
            self.assertTrue(done['verified'] and done['cleanup'])
            # Give the child time to exit while this loop cannot run its pidfd
            # callback. Python 3.12 used to reap it during close().
            time.sleep(.05)
            with self.assertNoLogs('asyncio', level='WARNING'):
                await client.close()
            self.assertEqual(client.process.returncode, 0)
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)
            for adapter_pid in adapter_pids:
                with self.assertRaises(ProcessLookupError):
                    os.kill(adapter_pid, 0)
            report = json.loads((folder / 'report.json').read_text())
            self.assertEqual(report['stop'], 'selector_stop')
            self.assertTrue(report['cleanup'])

    async def test_bad_replies_and_eof_never_execute(self):
        cases = [
            (lambda m: {**m, 'role': 'gm'}, 'invalid_api_reply'),
            (lambda m: {**m, 'action_id': 'unoffered'}, 'unknown_candidate'),
            (lambda m: {**m, 'decision_id': 'old'}, 'stale_decision'),
            (lambda m: {**m, 'action_id': 1}, 'invalid_api_reply'),
            (lambda m: [], 'invalid_api_reply'),
            (lambda m: b'{"action_id":"stop","action_id":"increment"}\n', 'invalid_api_reply'),
            (lambda m: b'{"action_id":NaN}\n', 'invalid_api_reply'),
            (lambda m: b'\xff\n', 'invalid_api_reply'),
            (lambda m: b'x' * 4097, 'invalid_api_reply'),
            (lambda m: b'{', 'invalid_api_reply'),
            (lambda m: None, 'client_disconnected'),
        ]
        for reply, reason in cases:
            with self.subTest(reason=reason):
                client, folder = await self.start()
                message = {'decision_id': client.observe()['decision_id'], 'action_id': 'increment'}
                answer = reply(message)
                if answer is not None:
                    raw = answer if isinstance(answer, bytes) else json.dumps(answer).encode() + b'\n'
                    client.process.stdin.write(raw)
                    await client.process.stdin.drain()
                client.process.stdin.close()
                done = await client._receive()
                report = await self.report(client, folder)
                self.assertEqual(done, completion(report))
                self.assertEqual(report['stop'], reason)
                self.assertEqual(report['attempted_inputs'], 0)
                self.assertEqual(self.receipts(folder)[0]['status'], reason)

    async def test_prequeued_reply_cannot_execute_twice(self):
        client, folder = await self.start()
        raw = json.dumps({'decision_id': client.observe()['decision_id'], 'action_id': 'increment'}).encode() + b'\n'
        client.process.stdin.write(raw * 2)
        await client.process.stdin.drain()
        self.assertEqual((await client._receive())['type'], 'observation')
        done = await client._receive()
        report = await self.report(client, folder)
        self.assertEqual(done['stop'], 'stale_decision')
        self.assertEqual(report['attempted_inputs'], 1)

    async def test_fragmented_reply_is_one_decision(self):
        client, folder = await self.start()
        raw = json.dumps({'decision_id': client.observe()['decision_id'], 'action_id': 'stop'}).encode() + b'\n'
        for chunk in (raw[:7], raw[7:31], raw[31:]):
            client.process.stdin.write(chunk)
            await client.process.stdin.drain()
            await asyncio.sleep(.01)
        self.assertEqual((await client._receive())['stop'], 'selector_stop')
        self.assertEqual((await self.report(client, folder))['attempted_inputs'], 0)

    async def test_timeout_and_sigint_with_stdin_open_keep_final_checks(self):
        for mode in ['timeout', 'sigint', 'client-close']:
            client, folder = await self.start(short=mode == 'timeout')
            if mode == 'sigint':
                client.process.stdin.write(b'{"partial":')
                await client.process.stdin.drain()
                client.process.send_signal(signal.SIGINT)
            elif mode == 'client-close':
                await asyncio.wait_for(client.close(), 5)
            report = await self.report(client, folder)
            self.assertEqual(report['stop'], 'operation_timeout' if mode == 'timeout' else 'cancelled')
            self.assertEqual(report['attempted_inputs'], 0)
            self.assertTrue(report['final']['ok'])
            self.assertEqual(self.receipts(folder)[0]['status'], 'interrupted')

    async def test_controller_rechecks_freshness_and_actual_effect(self):
        for mode, stop, inputs in [('stdio-stale', 'stale_observation', 0),
                                   ('busy', 'busy_refused', 0), ('omitted', 'evaluation_failed', 1)]:
            client, folder = await self.start(mode)
            done = await client.act('increment')
            report = await self.report(client, folder)
            self.assertEqual(done['stop'], stop)
            self.assertEqual(report['attempted_inputs'], inputs)
            if mode == 'omitted':
                self.assertFalse(done['verified'] or done['replayable'])

    async def test_fractional_values_replay_in_python_without_rounding(self):
        client, folder = await self.start('fractional')
        view = client.observe()['observation']
        self.assertEqual(view['clock'], .2)
        self.assertIsInstance(view['large'], float)
        self.assertEqual(json.dumps(view['negative_zero']), '-0.0')
        await client.act('increment')
        await client.act('stop')
        await self.report(client, folder)
        env = Fixture()
        observe = env.observe
        async def fractional():
            obs = await observe()
            return Observation(obs.environment, obs.epoch, obs.guard,
                {**obs.view, 'clock': .2, 'negative_zero': -0.0, 'large': 1e16}, obs.ready, obs.terminal)
        env.observe = fractional
        report = await run(env, self.root / 'fractional-replay',
            replay=json.loads((folder / 'replay.json').read_text()), limits=Limits(captures=0))
        self.assertTrue(report['replay_complete'])
        self.assertEqual(report['requests'], 0)


if __name__ == '__main__':
    unittest.main()
