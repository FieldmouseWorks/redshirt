"""Real subprocess transport and cross-runtime replay, optional until Rust build."""
import asyncio
from dataclasses import asdict
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

from redshirt import Limits, run
from redshirt.providers import Scripted
from test_runner import Fixture

BINARY = os.environ.get('REDSHIRT_BIN')
WORKER = Path(__file__).with_name('rust_fixture.py')


@unittest.skipUnless(BINARY, 'set REDSHIRT_BIN to the built Rust executable')
class RustProcessTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def command(self, name, mode='normal', *, replay=None, short=False, extra=()):
        limits = Limits(operation_seconds=.2 if short else 10, captures=0)
        config = self.root / (name + '-limits.json')
        config.write_text(json.dumps(asdict(limits)))
        return [BINARY, '--output', str(self.root / name), '--limits', str(config),
                *(['--replay', str(replay)] if replay else ['--remote-provider']),
                '--adapter', sys.executable, str(WORKER), mode, *map(str, extra)]

    def invoke(self, name, mode='normal', **kwargs):
        completed = subprocess.run(self.command(name, mode, **kwargs), capture_output=True, timeout=35)
        report = json.loads((self.root / name / 'report.json').read_text())
        self.assertTrue(report['cleanup'], completed.stderr)
        return report

    def test_cross_runtime_replay_and_checked_outcomes(self):
        python = asyncio.run(run(Fixture(), self.root / 'python', provider=Scripted(['increment', 'stop']), limits=Limits(captures=0)))
        rust = self.invoke('rust')
        self.assertEqual(python['evaluations'], rust['evaluations'])
        replay = self.invoke('from-python', replay=self.root / 'python/replay.json')
        self.assertTrue(replay['replay_complete'])
        self.assertEqual(replay['requests'], 0)
        saved = json.loads((self.root / 'rust/replay.json').read_text())
        result = asyncio.run(run(Fixture(), self.root / 'from-rust', replay=saved, limits=Limits(captures=0)))
        self.assertTrue(result['replay_complete'])

    def test_refusals_and_uncertain_attempt_survive_process_boundary(self):
        for mode, reason, inputs in [('busy', 'busy_refused', 0), ('stale', 'stale_observation', 0),
                                    ('invalid', 'adapter_error', 0), ('omitted', 'evaluation_failed', 1),
                                    ('uncertain', 'lost_receipt', 1)]:
            with self.subTest(mode=mode):
                result = self.invoke(mode, mode)
                self.assertEqual(result['stop'], reason)
                self.assertEqual(result['attempted_inputs'], inputs)
                if mode in ('omitted', 'uncertain'):
                    self.assertFalse(result['replayable'])

    def test_timeout_cancels_and_drains_before_final_check(self):
        result = self.invoke('timeout', 'slow', short=True)
        self.assertEqual(result['stop'], 'operation_timeout')
        self.assertEqual(result['attempted_inputs'], 0)
        self.assertTrue(result['final']['ok'])
        rows = [json.loads(s) for s in (self.root / 'timeout/events.jsonl').read_text().splitlines()]
        receipts = [r['data'] for r in rows if r['event'] == 'provider_receipt']
        self.assertEqual(receipts, [{'status': 'interrupted'}])

    def test_timeout_during_input_retains_uncertain_attempt(self):
        report = self.invoke('slow-execute', 'slow-execute', short=True)
        self.assertEqual(report['stop'], 'operation_timeout')
        self.assertEqual(report['attempted_inputs'], 1)
        self.assertTrue(report['final']['ok'])
        self.assertFalse(report['replayable'])

    def test_sigint_preserves_finalization_and_reaps_worker(self):
        marker = self.root / 'selecting'
        process = subprocess.Popen(self.command('cancel', 'cancel', extra=[marker]), stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 5
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(marker.exists())
            process.send_signal(signal.SIGINT)
            process.communicate(timeout=5)
            report = json.loads((self.root / 'cancel/report.json').read_text())
            self.assertEqual(report['stop'], 'cancelled')
            self.assertEqual(report['attempted_inputs'], 0)
            self.assertTrue(report['cleanup'] and report['final']['ok'])
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()

    def test_hidden_state_excluded_and_unicode_replay(self):
        for name in ('hidden-one', 'hidden-two'):
            self.invoke(name, name)
        def request(name):
            rows = [json.loads(s) for s in (self.root / name / 'events.jsonl').read_text().splitlines()]
            return next(r['data'] for r in rows if r['event'] == 'request')
        self.assertEqual(request('hidden-one'), request('hidden-two'))
        self.invoke('unicode', 'unicode')
        replay = self.invoke('unicode-replay', 'unicode', replay=self.root / 'unicode/replay.json')
        self.assertTrue(replay['replay_complete'])


if __name__ == '__main__':
    unittest.main()
