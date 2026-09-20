"""Independent synthetic task checks for the Rust comparison process test."""
import asyncio
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from redshirt.adapter_stdio import serve_stdio
from test_runner import Fixture


class Task(Fixture):
    async def evaluate(self, phase, operation):
        verdict = await super().evaluate(phase, operation)
        verdict.checks['comparison'] = {
            'complete': self.value == 1,
            'useful_action': self.value == 1 if phase == 'after' else None,
        }
        return verdict


class Baseline:
    name = 'synthetic-visible-baseline'
    async def select(self, request):
        return 'increment' if request['observation']['counter'] == 0 else 'stop'


async def main():
    assert 'TYPESAFE_API_KEY' not in os.environ
    await serve_stdio(Task(), provider=Baseline())


if __name__ == '__main__':
    asyncio.run(main())
