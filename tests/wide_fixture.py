"""Synthetic wide counter menus for controller/provider/pipe contract checks."""
import argparse
import asyncio
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from redshirt import Candidate, Limits
from redshirt.adapter_stdio import serve_stdio
from redshirt.interaction import serve_stdio as serve_interaction
from redshirt.jev import Jev, MODEL
from test_runner import Fixture

WIDE = dict(candidates=254, candidate_bytes=65536, decision_bytes=32768)


class WideFixture(Fixture):
    def __init__(self, count=200, width=100, id_width=0):
        super().__init__()
        self.count, self.width, self.id_width = count, width, id_width
        self.identity = {**self.identity, 'menu': [count, width, id_width]}

    def choice(self, index):
        return ('option_' + str(index)).ljust(self.id_width, 'x')

    def candidates(self, observation):
        return [Candidate(self.choice(i), 'x' * self.width, {'button': 'increment', 'index': i})
                for i in range(self.count)]


def mock(env, *, request_bytes=65536):
    async def transport(body):
        request = json.loads(body)['state']
        choice = env.choice(env.count - 1) if request['observation']['counter'] == 0 else 'stop'
        answer = {'model': MODEL, 'answers': {'action': {'type': 'choice', 'choice': choice,
                  'probabilities': {k: float(k == choice) for k in request['candidates']}, 'confidence': 1.0}},
                  'usage': {'input_tokens': 100, 'output_tokens': 10}}
        return 200, json.dumps(answer).encode()
    return Jev(transport, request_limit=2, request_bytes=request_bytes)


async def main():
    p = argparse.ArgumentParser()
    p.add_argument('--count', type=int, default=200)
    p.add_argument('--width', type=int, default=100)
    p.add_argument('--id-width', type=int, default=0)
    p.add_argument('--interaction-output', type=Path)
    args = p.parse_args()
    env = WideFixture(args.count, args.width, args.id_width)
    if args.interaction_output:
        await serve_interaction(env, args.interaction_output, limits=Limits(captures=0, **WIDE))
    else:
        # The method worker never delegates the episode back to Python.
        import redshirt.runner
        async def forbidden(*args, **kwargs):
            raise AssertionError('nested_python_controller')
        redshirt.runner.run = forbidden
        await serve_stdio(env, provider=mock(env))


if __name__ == '__main__':
    asyncio.run(main())
