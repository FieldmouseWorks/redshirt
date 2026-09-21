"""Wide synthetic menus through the Rust process owner and unchanged Python client."""
import asyncio
from dataclasses import asdict
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

from redshirt import Limits, run
from redshirt.evidence import encoded
from redshirt.interaction import InteractionClient
from redshirt.providers import Scripted
from wide_fixture import WideFixture, WIDE

BINARY = os.environ.get('REDSHIRT_BIN')
WORKER = Path(__file__).with_name('wide_fixture.py')


@unittest.skipUnless(BINARY, 'set REDSHIRT_BIN to the built Rust executable')
class RustWideTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name); self.serial = 0

    async def invoke(self, *, count=200, width=100, limits=None, replay=None, script=None):
        self.serial += 1; folder = self.root / str(self.serial)
        args = [BINARY, '--output', str(folder), '--limits-json', json.dumps(asdict(limits or Limits(captures=0, **WIDE)))]
        if replay: args += ['--replay', str(replay)]
        elif script is not None:
            path = self.root / (str(self.serial) + '-choices.json'); path.write_text(json.dumps(script)); args += ['--script', str(path)]
        else: args += ['--remote-provider']
        args += ['--adapter', sys.executable, str(WORKER), '--count', str(count), '--width', str(width)]
        proc = await asyncio.create_subprocess_exec(*args, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
        stdout, stderr = await asyncio.wait_for(proc.communicate(), 15)
        report = json.loads((folder / 'report.json').read_text())
        self.assertTrue(report['cleanup'] and report['final']['ok'], (stdout, stderr))
        return report, folder

    async def test_default_and_max_count_limits_then_scripted_replay(self):
        for count, limits, stop, inputs in [(96, Limits(captures=0), 'selector_stop', 1),
                                          (97, Limits(captures=0), 'invalid_candidates', 0),
                                          (254, Limits(captures=0, **WIDE), 'selector_stop', 1),
                                          (255, Limits(captures=0, **WIDE), 'invalid_candidates', 0)]:
            report, folder = await self.invoke(count=count,width=1,limits=limits,script=['option_'+str(count-1),'stop'])
            self.assertEqual((report['stop'],report['attempted_inputs']),(stop,inputs))
            if inputs:
                replay, _ = await self.invoke(count=count,width=1,limits=limits,replay=folder/'replay.json')
                self.assertTrue(replay['replay_complete']);self.assertEqual(replay['requests'],0)

    async def test_exact_encoded_table_and_decision_limits(self):
        env=WideFixture(8,200);obs=await env.observe();cs=env.candidates(obs)
        request={'version':1,'observation':obs.view,'candidates':{**{c.id:c.description for c in cs},'stop':'Stop the experiment.'},'remaining_inputs':24}
        for field,size,reason in [('candidate_bytes',len(encoded([asdict(c) for c in cs])),'candidate_size'),('decision_bytes',len(encoded(request)),'observation_size')]:
            for allowed in (size,size-1):
                report,_=await self.invoke(count=8,width=200,limits=Limits(captures=0,**{**WIDE,field:allowed}),script=['stop'])
                self.assertEqual(report['stop'],'selector_stop' if allowed==size else reason)
                self.assertEqual(report['requests'],int(allowed==size));self.assertEqual(report['attempted_inputs'],0)

    async def test_wide_mock_and_both_cross_runtime_replay_directions(self):
        rust,folder=await self.invoke()
        self.assertEqual((rust['stop'],rust['requests'],rust['attempted_inputs']),('selector_stop',2,1))
        events=[json.loads(s) for s in (folder/'events.jsonl').read_text().splitlines()]
        receipts=[r['data'] for r in events if r['event']=='provider_receipt'];self.assertEqual(len(receipts),2)
        for row in receipts:
            self.assertEqual(len(row['request']['state']['candidates']),201)
            self.assertEqual(row['request']['state']['candidates'],row['request']['questions']['action']['criteria'])
            self.assertGreater(len(encoded(row['request'])),16384)
        py=await run(WideFixture(),self.root/'py-replay',replay=json.loads((folder/'replay.json').read_text()),limits=Limits(captures=0,**WIDE))
        self.assertTrue(py['replay_complete'] and py['cleanup']);self.assertEqual(py['requests'],0)
        await run(WideFixture(),self.root/'py',provider=Scripted(['option_199','stop']),limits=Limits(captures=0,**WIDE))
        replay,_=await self.invoke(replay=self.root/'py/replay.json')
        self.assertTrue(replay['replay_complete']);self.assertEqual(replay['requests'],0)
        self.assertEqual(py['evaluations'],replay['evaluations'])

    async def test_large_json_frame_keeps_the_complete_schema_and_old_client(self):
        env=WideFixture(200,30,70)
        args=[BINARY,'--output',str(self.root/'stdio'),'--stdio','--limits-json',json.dumps(asdict(Limits(captures=0,**WIDE))),
              '--adapter',sys.executable,str(WORKER),'--count','200','--width','30','--id-width','70']
        client=await InteractionClient.start(*args);self.addAsyncCleanup(client.close)
        py=await InteractionClient.start(sys.executable,str(WORKER),'--count','200','--width','30','--id-width','70','--interaction-output',str(self.root/'py-stdio'))
        self.addAsyncCleanup(py.close)
        for action in [env.choice(199),'stop']:
            a=client.observe();b=py.observe();a.pop('decision_id');b.pop('decision_id')
            self.assertEqual(a,b);self.assertGreater(len(encoded(a)),32768)
            self.assertEqual(set(a['tool']['parameters']['properties']['action_id']['enum']),set(a['candidates']))
            self.assertEqual(len(a['candidates']),201)
            await client.act(action);await py.act(action)
        self.assertTrue(client.observe()['verified'] and client.observe()['cleanup'])
        self.assertEqual(client.observe(),py.observe())
        await asyncio.wait_for(client.process.wait(),5);await asyncio.wait_for(py.process.wait(),5)

    async def test_malformed_new_limits_refuse_before_an_adapter_is_started(self):
        for field,values in [('candidates',[0,255,True,2.0]),('candidate_bytes',[1023,65537]),('decision_bytes',[1023,32769])]:
            for value in values:
                args=[BINARY,'--output',str(self.root/'never-created'),'--limits-json',json.dumps({field:value}),
                      '--remote-provider','--adapter','/no/such/adapter']
                proc=await asyncio.create_subprocess_exec(*args,stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.PIPE)
                stdout,stderr=await proc.communicate();self.assertNotEqual(proc.returncode,0)
                self.assertIn(b'invalid_limits',stderr);self.assertNotIn(b'adapter_spawn',stderr)
                self.assertFalse((self.root/'never-created').exists())
