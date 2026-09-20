"""Model-free selectors and an injected transport seam; no network implementation."""
import asyncio
import json
import random


class Scripted:
    name = "scripted"

    def __init__(self, ids):
        self.ids = iter(ids)
        self.calls = 0

    async def select(self, request):
        self.calls += 1
        return next(self.ids, "stop")


class Seeded:
    name = "seeded"

    def __init__(self, seed):
        self.rng = random.Random(seed)
        self.calls = 0

    async def select(self, request):
        self.calls += 1
        return self.rng.choice(list(request["candidates"]))


class MockTransport:
    """An explicitly supplied async callable, with no retries or fallback.

    Transport receives only the selector request. Parsing this response grants
    no authority; the controller still regenerates and validates candidates.
    """
    name = "mock-transport"

    def __init__(self, transport):
        self.transport = transport
        self.calls = 0
        self._busy = False

    async def select(self, request):
        if self._busy:
            raise ValueError("provider_busy")
        self._busy = True
        try:
            self.calls += 1
            raw = await self.transport(request)
            if not isinstance(raw, bytes) or len(raw) > 4096:
                raise ValueError("invalid_response_size")

            def unique(pairs):
                out = {}
                for key, value in pairs:
                    if key in out:
                        raise ValueError("duplicate_response_key")
                    out[key] = value
                return out

            reply = json.loads(raw, object_pairs_hook=unique)
            if not isinstance(reply, dict) or set(reply) != {"candidate_id"} or not isinstance(reply["candidate_id"], str):
                raise ValueError("invalid_response")
            return reply["candidate_id"]
        finally:
            self._busy = False
