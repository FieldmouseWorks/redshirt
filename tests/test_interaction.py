"""Local protocol contracts, with synthetic data and no external service."""
import asyncio
import json
from pathlib import Path
import sys
import tempfile
import unittest

from redshirt import Limits, run
from redshirt.interaction import Interactive, InteractionClient, choice_tool, completion
from test_runner import Fixture


class Writer:
    def __init__(self, reader, reply=None):
        self.reader, self.reply, self.messages = reader, reply, []

    def write(self, raw):
        message = json.loads(raw)
        self.messages.append(message)
        if self.reply:
            answer = self.reply(message)
            if answer is None:
                self.reader.feed_eof()
            else:
                self.reader.feed_data(answer)

    async def drain(self):
        pass


def response(message, action='increment', **extra):
    return json.dumps({'decision_id': message['decision_id'], 'action_id': action, **extra}).encode() + b'\n'


class InteractionTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.serial = 0

    def output(self):
        self.serial += 1
        return Path(self.tmp.name) / str(self.serial)

    async def episode(self, reply, env=None, **kwargs):
        reader = asyncio.StreamReader(limit=4096)
        writer = Writer(reader, reply)
        env = env or Fixture()
        folder = self.output()
        report = await run(env, folder, provider=Interactive(reader, writer),
                           limits=Limits(captures=0), **kwargs)
        self.assertTrue(env.closed and env.finalized)
        self.assertTrue(report['cleanup'])
        return report, writer.messages, folder

    async def test_json_decisions_and_concrete_replay(self):
        ids = iter(['increment', 'increment', 'stop'])
        report, frames, folder = await self.episode(lambda m: response(m, next(ids)))
        self.assertEqual(report['stop'], 'selector_stop')
        self.assertEqual([m['observation']['counter'] for m in frames], [0, 1, 2])
        self.assertEqual(len({m['decision_id'] for m in frames}), 3)
        self.assertTrue(all(m['tool'] == choice_tool(m['candidates']) for m in frames))
        self.assertNotIn('hidden', json.dumps(frames))
        self.assertNotIn('checks', json.dumps(completion(report)))
        self.assertNotIn('operations', completion(report))
        saved = json.loads((folder / 'replay.json').read_text())
        replay = await run(Fixture(), self.output(), replay=saved, limits=Limits(captures=0))
        self.assertTrue(replay['replay_complete'])
        self.assertEqual(replay['requests'], 0)

    async def test_bad_replies_never_execute(self):
        cases = [
            (lambda m: response(m, role='admin'), 'invalid_api_reply'),
            (lambda m: response(m, 'unoffered'), 'unknown_candidate'),
            (lambda m: b'{"decision_id":"old","action_id":"increment"}\n', 'stale_decision'),
            (lambda m: b'{"action_id":"stop","action_id":"increment"}\n', 'invalid_api_reply'),
            (lambda m: b'{"action_id":NaN}\n', 'invalid_api_reply'),
            (lambda m: b'x' * 5000 + b'\n', 'invalid_api_reply'),
            (lambda m: b'\xff\n', 'invalid_api_reply'),
            (lambda m: None, 'client_disconnected'),
        ]
        for reply, reason in cases:
            with self.subTest(reason=reason):
                report, _, _ = await self.episode(reply)
                self.assertEqual(report['stop'], reason)
                self.assertEqual(report['attempted_inputs'], 0)

    async def test_prequeued_reply_cannot_execute_twice(self):
        sent = False
        def reply(message):
            nonlocal sent
            if sent:
                return b''
            sent = True
            return response(message) * 2
        report, _, _ = await self.episode(reply)
        self.assertEqual(report['stop'], 'stale_decision')
        self.assertEqual(report['attempted_inputs'], 1)

    async def test_cancellation_while_waiting_preserves_checks(self):
        cancel = asyncio.Event()
        def reply(message):
            cancel.set()
            return b''
        report, _, _ = await self.episode(reply, cancel=cancel)
        self.assertEqual(report['stop'], 'cancelled')
        self.assertEqual(report['attempted_inputs'], 0)

    async def test_environment_freshness_still_owned_by_runner(self):
        env = Fixture()
        def reply(message):
            env.revision += 1
            return response(message)
        report, _, _ = await self.episode(reply, env=env)
        self.assertEqual(report['stop'], 'stale_observation')
        self.assertEqual(report['attempted_inputs'], 0)

    async def test_failed_effect_ends_session_without_second_choice(self):
        report, frames, _ = await self.episode(response, env=Fixture(omit_input=True))
        self.assertEqual(report['stop'], 'evaluation_failed')
        self.assertEqual(len(frames), 1)
        self.assertFalse(completion(report)['verified'])

    async def test_real_pipes_and_python_client(self):
        async with await InteractionClient.start(sys.executable,
                str(Path(__file__).with_name('interactive_fixture.py')), str(self.output())) as client:
            initial = client.observe()
            self.assertEqual(initial['observation'], {'counter': 0})
            initial['candidates'].clear()
            self.assertIn('increment', client.observe()['candidates'])
            frame = await client.act('increment')
            self.assertEqual(frame['observation'], {'counter': 1})
            done = await client.act('stop')
            self.assertEqual(done['stop'], 'selector_stop')
            self.assertTrue(done['verified'] and done['cleanup'])
            with self.assertRaisesRegex(ValueError, 'session_finished'):
                await client.act('increment')

    async def test_client_close_cancels_owned_process(self):
        folder = self.output()
        client = await InteractionClient.start(sys.executable,
            str(Path(__file__).with_name('interactive_fixture.py')), str(folder))
        await client.close()
        report = json.loads((folder / 'report.json').read_text())
        self.assertEqual(report['stop'], 'cancelled')
        self.assertTrue(report['cleanup'])
        self.assertIsNotNone(client.process.returncode)


if __name__ == '__main__':
    unittest.main()
