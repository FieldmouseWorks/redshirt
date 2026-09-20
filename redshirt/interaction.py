"""Local JSON-lines decisions and a small async client. No environment rules.

The existing runner owns execution, timeouts, checks, evidence and replay. This
transport only exposes its audited selector request and accepts one choice.
"""
import asyncio
import json
import secrets
import signal
import sys

from .evidence import encoded
from .runner import Limits, Stop, run

MAX_MESSAGE = 32768
MAX_REPLY = 4096


def choice_tool(candidates):
    """Provider-neutral function definition; the controller keeps decision IDs."""
    return {"name": "choose_action", "description": "Choose one currently offered action.",
            "parameters": {"type": "object", "properties": {
                "action_id": {"type": "string", "enum": list(candidates)}},
                "required": ["action_id"], "additionalProperties": False}}


def _decode(raw, limit):
    def nonfinite(_):
        raise ValueError("nonfinite")

    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate_key")
            result[key] = value
        return result
    if not raw or len(raw) > limit or not raw.endswith(b"\n"):
        raise ValueError("message_size_or_framing")
    result = json.loads(raw, object_pairs_hook=unique, parse_constant=nonfinite)
    if not isinstance(result, dict):
        raise ValueError("message_shape")
    return result


async def _send(writer, message):
    raw = encoded(message) + b"\n"
    if len(raw) > MAX_MESSAGE:
        raise ValueError("message_size")
    writer.write(raw)
    await writer.drain()


class Interactive:
    """One response per observation; EOF or invalid replies stop, never retry."""
    name = "local-jsonl"
    evidence_limit = 2048

    def __init__(self, reader, writer):
        self.reader, self.writer = reader, writer
        self._busy = False
        self._receipts = []

    async def select(self, request):
        if self._busy:
            raise Stop("provider_busy")
        self._busy = True
        decision = secrets.token_hex(16)
        receipt = {"decision_id": decision, "status": "interrupted"}
        try:
            await _send(self.writer, {**request, "type": "observation", "decision_id": decision,
                                      "tool": choice_tool(request["candidates"])})
            try:
                raw = await self.reader.readline()
                if not raw:
                    raise Stop("client_disconnected")
                reply = _decode(raw, MAX_REPLY)
            except (ValueError, UnicodeError):
                raise Stop("invalid_api_reply") from None
            if (set(reply) != {"decision_id", "action_id"}
                    or not isinstance(reply["decision_id"], str)
                    or not isinstance(reply["action_id"], str)):
                raise Stop("invalid_api_reply")
            if reply["decision_id"] != decision:
                raise Stop("stale_decision")
            if reply["action_id"] not in request["candidates"]:
                raise Stop("unknown_candidate")
            receipt.update(status="selected", action_id=reply["action_id"])
            return reply["action_id"]
        except Stop as exc:
            receipt["status"] = str(exc)
            raise
        except (BrokenPipeError, ConnectionError):
            receipt["status"] = "client_disconnected"
            raise Stop("client_disconnected") from None
        finally:
            self._receipts.append(receipt)
            self._busy = False

    def take_evidence(self):
        receipts, self._receipts = self._receipts, []
        return receipts


def completion(report):
    """No evaluator internals, private operations or artifact paths on the wire."""
    return {"version": 1, "type": "done", "stop": report["stop"],
            "requests": report["requests"], "attempted_inputs": report["attempted_inputs"],
            "verified": report["final"]["ok"], "cleanup": report["cleanup"],
            "replayable": report["replayable"]}


async def serve_stdio(adapter, output, *, limits=Limits(captures=0), cancel=None):
    """Run one bounded episode over local pipes. Stdout is exclusively JSONL.

    POSIX pipe transport; no port, remote browser or model credential is opened.
    Decision time continues to advance in real-time environments.
    """
    loop = asyncio.get_running_loop()
    reader = asyncio.StreamReader(limit=MAX_REPLY)
    incoming, _ = await loop.connect_read_pipe(lambda: asyncio.StreamReaderProtocol(reader), sys.stdin.buffer)
    outgoing = None
    try:
        outgoing, protocol = await loop.connect_write_pipe(
            lambda: asyncio.streams.FlowControlMixin(loop=loop), sys.stdout.buffer)
        writer = asyncio.StreamWriter(outgoing, protocol, None, loop)
        report = await run(adapter, output, provider=Interactive(reader, writer), limits=limits, cancel=cancel)
        try:
            await asyncio.wait_for(_send(writer, completion(report)), 5)
        except (TimeoutError, BrokenPipeError, ConnectionError):
            pass  # The runner has already preserved checks and finalized evidence.
        return report
    finally:
        incoming.close()
        if outgoing:
            outgoing.close()


class InteractionClient:
    """Async client for an operator-selected local adapter process.

    Call observe(), choose from candidates/tool, then await act(action_id).
    The client inserts the decision token. It never accepts model-supplied argv.
    act() returns the next observation or a terminal done envelope.
    """
    def __init__(self, process):
        self.process = process
        self._frame = None
        self._busy = False

    @classmethod
    async def start(cls, *argv, cwd=None, env=None):
        process = await asyncio.create_subprocess_exec(*argv, cwd=cwd, env=env,
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, limit=MAX_MESSAGE)
        client = cls(process)
        try:
            client._frame = await client._receive()
            return client
        except BaseException:
            await client.close()
            raise

    async def _receive(self):
        raw = await asyncio.wait_for(self.process.stdout.readline(), 35)
        message = _decode(raw, MAX_MESSAGE)
        if message.get("version") != 1 or message.get("type") not in ("observation", "done"):
            raise ValueError("invalid_api_message")
        return message

    def observe(self):
        if self._frame is None:
            raise ValueError("no_current_observation")
        return json.loads(encoded(self._frame))

    async def act(self, action_id):
        if self._busy:
            raise ValueError("client_busy")
        frame = self.observe()
        if frame["type"] != "observation":
            raise ValueError("session_finished")
        if not isinstance(action_id, str) or action_id not in frame["candidates"]:
            raise ValueError("unknown_candidate")
        self._busy, self._frame = True, None
        try:
            await _send(self.process.stdin, {"decision_id": frame["decision_id"], "action_id": action_id})
            self._frame = await self._receive()
            return self.observe()
        finally:
            self._busy = False

    async def close(self):
        if self.process.returncode is None:
            try:
                self.process.send_signal(signal.SIGINT)
            except ProcessLookupError:
                pass
            try:
                await asyncio.wait_for(self.process.wait(), 30)
            except TimeoutError:
                self.process.kill()
                await self.process.wait()
        if self.process.stdin:
            self.process.stdin.close()

    async def __aenter__(self):
        return self

    async def __aexit__(self, *_):
        await self.close()
