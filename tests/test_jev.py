import asyncio
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from redshirt import Limits, run
from redshirt.jev import Jev, MODEL, decode, validate
from test_runner import Fixture

REQUEST = {"version": 1, "observation": {"objective": "Increment once."},
           "candidates": {"increment": "Increment.", "stop": "Stop."}, "remaining_inputs": 24}


def response(choice="increment"):
    return {"model": MODEL, "answers": {"action": {"type": "choice", "choice": choice,
            "probabilities": {"increment": .8 if choice == "increment" else .2,
                              "stop": .8 if choice == "stop" else .2}, "confidence": .7}},
            "usage": {"input_tokens": 100, "output_tokens": 12}}


class JevTests(unittest.IsolatedAsyncioTestCase):
    async def test_evidence_reservation_refuses_before_transport(self):
        provider = Jev(lambda body: self.fail('must not dispatch'))
        provider.evidence_limit = 131072
        with tempfile.TemporaryDirectory() as tmp:
            report = await run(Fixture(), Path(tmp) / 'run', provider=provider,
                               limits=Limits(evidence_bytes=262144))
        self.assertEqual(report['stop'], 'evidence_budget')
        self.assertEqual(provider.calls, 0)
        self.assertEqual(report['requests'], 0)
        self.assertTrue(report['final']['ok'] and report['cleanup'])

    async def test_broad_menu_is_preserved_without_model_pruning(self):
        request = copy.deepcopy(REQUEST)
        request['candidates'] = {str(n): 'Choose visible target ' + str(n) for n in range(96)}
        request['candidates']['stop'] = 'Stop.'
        async def transport(body):
            self.assertEqual(json.loads(body)['questions']['action']['criteria'], request['candidates'])
            reply = response()
            reply['answers']['action'].update(choice='73', probabilities={k: float(k == '73') for k in request['candidates']})
            return 200, json.dumps(reply).encode()
        self.assertEqual(await Jev(transport).select(request), '73')

    async def test_absolute_deadline_cancels_transport(self):
        stopped = asyncio.Event()
        async def transport(body):
            try: await asyncio.sleep(60)
            finally: stopped.set()
        provider = Jev(transport)
        with self.assertRaises(TimeoutError):
            await asyncio.wait_for(provider.select(REQUEST), 6)
        self.assertTrue(stopped.is_set())
        self.assertEqual(provider.calls, 1)
        self.assertEqual(provider.take_evidence()[0]['error_type'], 'TimeoutError')

    async def test_https_transport_configuration_without_network(self):
        try: import httpx
        except ImportError: self.skipTest('optional httpx extra not installed')
        from redshirt.jev import ENDPOINT
        received, settings = [], []
        def handler(request):
            received.append(request)
            return httpx.Response(200, json=response())
        client = httpx.AsyncClient
        def configured(**kwargs):
            settings.append(kwargs)
            return client(**kwargs)
        provider = Jev.live('synthetic-only-key')
        with patch.object(httpx, 'AsyncHTTPTransport', return_value=httpx.MockTransport(handler)) as transport, patch.object(httpx, 'AsyncClient', side_effect=configured):
            self.assertEqual(await provider.select(REQUEST), 'increment')
        transport.assert_called_once_with(retries=0)
        self.assertEqual(len(received), 1)
        self.assertEqual(str(received[0].url), ENDPOINT)
        self.assertEqual(received[0].headers['Authorization'], 'Bearer synthetic-only-key')
        self.assertFalse(settings[0]['trust_env'])
        self.assertFalse(settings[0]['follow_redirects'])
        self.assertNotIn('synthetic-only-key', json.dumps(provider.take_evidence()))

    async def test_exact_request_receipt_and_budget(self):
        bodies = []
        async def transport(body):
            bodies.append(json.loads(body))
            return 200, json.dumps(response()).encode()
        provider = Jev(transport, request_limit=1)
        self.assertEqual(await provider.select(REQUEST), "increment")
        self.assertEqual(bodies[0]["state"], REQUEST)
        self.assertEqual(bodies[0]["questions"]["action"]["criteria"], REQUEST["candidates"])
        receipts = provider.take_evidence()
        self.assertEqual(receipts[0]["outcome"], "accepted")
        self.assertEqual(receipts[0]["usage"]["input_tokens"], 100)
        self.assertIsNone(receipts[0]["billed_usd"])
        with self.assertRaisesRegex(ValueError, "provider_request_budget"):
            await provider.select(REQUEST)
        self.assertEqual(len(bodies), 1)
        self.assertEqual(provider.take_evidence(), [])

    def test_probability_policy_and_strict_shapes(self):
        valid = response()
        approximate = copy.deepcopy(valid)
        approximate["answers"]["action"]["probabilities"]["stop"] = .195
        self.assertEqual(validate(approximate, REQUEST["candidates"])[1]["classification"], "accepted_approximate")
        self.assertEqual(approximate["answers"]["action"]["probabilities"]["stop"], .195)
        for mutate in [
            lambda r: r.update(model="jev-latest"),
            lambda r: r.pop("usage"),
            lambda r: r["usage"].update(input_tokens=True),
            lambda r: r["answers"]["action"].update(choice="other"),
            lambda r: r["answers"]["action"].update(choice="stop"),
            lambda r: r["answers"]["action"].update(confidence=float("nan")),
            lambda r: r["answers"]["action"]["probabilities"].update(stop=.18),
            lambda r: r["answers"]["action"]["probabilities"].update(other=0),
            lambda r: r["answers"]["action"]["probabilities"].update(stop=True),
            lambda r: r["answers"].update(extra={}),
        ]:
            changed = copy.deepcopy(valid)
            mutate(changed)
            with self.assertRaises(ValueError):
                validate(changed, REQUEST["candidates"])
        for raw in [b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}']:
            with self.assertRaises(ValueError): decode(raw)

    async def test_failure_receipt_preserved_by_runner(self):
        for status, raw in [(429, b'{"error":"busy"}'), (200, b'{"broken":')]:
            async def transport(body): return status, raw
            with tempfile.TemporaryDirectory() as tmp:
                output = Path(tmp) / "run"
                report = await run(Fixture(), output, provider=Jev(transport))
                self.assertEqual(report["stop"], "harness_error")
                self.assertEqual(report["attempted_inputs"], 0)
                self.assertTrue(report["final"]["ok"] and report["cleanup"])
                events = [json.loads(s) for s in (output / "events.jsonl").read_text().splitlines()]
                receipts = [e["data"] for e in events if e["event"] == "provider_receipt"]
                self.assertEqual(len(receipts), 1)
                self.assertEqual(receipts[0]["http_status"], status)
                self.assertEqual(receipts[0]["outcome"], "failed")

    async def test_cancellation_and_concurrent_request(self):
        entered, cancel = asyncio.Event(), asyncio.Event()
        async def transport(body):
            entered.set()
            await asyncio.sleep(60)
        provider = Jev(transport)
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "run"
            task = asyncio.create_task(run(Fixture(), output, provider=provider, cancel=cancel))
            await entered.wait()
            with self.assertRaisesRegex(ValueError, "provider_busy"):
                await provider.select(REQUEST)
            cancel.set()
            report = await asyncio.wait_for(task, 1)
            self.assertEqual(report["stop"], "cancelled")
            self.assertEqual(provider.calls, 1)
            events = (output / "events.jsonl").read_text()
            self.assertIn('"outcome":"cancelled"', events)
            self.assertEqual(report["attempted_inputs"], 0)

    async def test_timeout_no_retry_and_redaction(self):
        async def timeout(body): raise TimeoutError("sensitive error text")
        provider = Jev(timeout)
        with self.assertRaises(TimeoutError): await provider.select(REQUEST)
        receipt = provider.take_evidence()[0]
        self.assertNotIn("sensitive", json.dumps(receipt))
        self.assertEqual(provider.calls, 1)
        async def reflected(body): return 401, b'{"error":"synthetic-key"}'
        provider = Jev(reflected)
        provider._secret = "synthetic-key"
        with self.assertRaises(ValueError): await provider.select(REQUEST)
        self.assertNotIn("synthetic-key", json.dumps(provider.take_evidence()))

    async def test_injected_jev_choices_replay_without_provider(self):
        choices = iter(["increment", "stop"])
        async def transport(body): return 200, json.dumps(response(next(choices))).encode()
        with tempfile.TemporaryDirectory() as tmp:
            first = Path(tmp) / "first"
            report = await run(Fixture(), first, provider=Jev(transport))
            self.assertEqual(report["attempted_inputs"], 1)
            replay = await run(Fixture(), Path(tmp) / "replay", replay=json.loads((first / "replay.json").read_text()))
            self.assertTrue(replay["replay_complete"])
            self.assertEqual(replay["requests"], 0)
