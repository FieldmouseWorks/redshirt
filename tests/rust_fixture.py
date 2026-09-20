"""Synthetic process worker; contains no project rules or data."""
import asyncio
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from redshirt.adapter_stdio import serve_stdio
from redshirt import Observation, Stop
from redshirt.providers import MockTransport
from test_runner import Fixture


async def main():
    mode = sys.argv[1]
    env = Fixture(omit_input=mode == 'omitted', hidden=mode)
    if mode == 'attach':
        env.setup_mode = 'attach'
    if mode == 'busy':
        env.ready = False
    if mode in ('unicode', 'fractional'):
        original = env.observe
        async def observe():
            obs = await original()
            extra = {'label': 'é😀'} if mode == 'unicode' else {'clock': 0.2, 'negative_zero': -0.0, 'large': 1e16}
            return Observation(obs.environment, obs.epoch, obs.guard, {**obs.view, **extra}, obs.ready, obs.terminal)
        env.observe = observe
    if mode == 'stdio-stale':
        original = env.observe
        observations = 0
        async def observe():
            nonlocal observations
            observations += 1
            # Baseline, offered frame, then the controller's pre-input recheck.
            if observations == 3:
                env.revision += 1
            return await original()
        env.observe = observe
    if mode in ('uncertain', 'slow-execute'):
        original = env.execute
        async def execute(operation):
            await original(operation)
            if mode == 'slow-execute':
                await asyncio.sleep(20)
            raise Stop('lost_receipt')
        env.execute = execute
    calls = 0
    async def transport(request):
        nonlocal calls
        calls += 1
        if mode == 'stale':
            env.revision += 1
        if mode in ('slow', 'cancel'):
            if len(sys.argv) > 2:
                Path(sys.argv[2]).write_text('selecting')
            await asyncio.sleep(20)
        if mode == 'invalid':
            return b'{"candidate_id":"increment","command":"forbidden"}'
        return b'{"candidate_id":"increment"}' if calls == 1 else b'{"candidate_id":"stop"}'
    # Prove the worker cannot silently delegate control to the Python runner.
    import redshirt.runner
    async def forbidden(*args, **kwargs):
        raise AssertionError('nested_python_controller')
    redshirt.runner.run = forbidden
    class Audited(MockTransport):
        def __init__(self):
            super().__init__(transport)
            self.receipts = []

        async def select(self, request):
            status = 'interrupted'
            try:
                result = await super().select(request)
                status = 'selected'
                return result
            finally:
                self.receipts.append({'status': status})

        def take_evidence(self):
            rows, self.receipts = self.receipts, []
            return rows
    await serve_stdio(env, provider=Audited())


if __name__ == '__main__':
    asyncio.run(main())
