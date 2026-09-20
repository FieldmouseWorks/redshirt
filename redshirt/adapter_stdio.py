"""Trusted adapter methods over local pipes; no episode/controller loop here.

The Rust owner supplies sequencing, authorization, timeouts, evidence and replay.
Cancellation joins the active method before another method can start. Emergency
EOF cleanup closes owned adapter resources; stdout contains protocol frames only.
"""
import asyncio
from dataclasses import asdict
import signal
import sys

from .evidence import encoded
from .interaction import _decode
from .runner import Stop

MAX_REQUEST = 32768
MAX_RESPONSE = 196608


async def serve(adapter, reader, writer, *, provider=None):
    active = incoming = None
    active_id = last_id = 0
    closed = False

    def descriptor():
        return {'identity': adapter.identity, 'setup_mode': getattr(adapter, 'setup_mode', 'reset')}

    async def invoke(message):
        nonlocal closed
        method, params = message['method'], message['params']
        response = {'version': 1, 'id': message['id'], 'receipts': []}
        try:
            if method in ('describe', 'reset', 'observe', 'verify', 'close') and params is not None:
                raise Stop('invalid_method_params')
            if method == 'describe':
                value = descriptor()
            elif method == 'reset':
                value = await adapter.reset()
            elif method == 'observe':
                observation = await adapter.observe()
                value = {'observation': asdict(observation),
                         'candidates': [asdict(c) for c in adapter.candidates(observation)]}
            elif method == 'verify':
                await adapter.verify()
                value = descriptor()
            elif method == 'execute':
                if not isinstance(params, dict):
                    raise Stop('invalid_method_params')
                value = await adapter.execute(params)
            elif method == 'evaluate':
                if (not isinstance(params, dict) or set(params) != {'phase', 'operation'}
                        or params['phase'] not in ('reset', 'after', 'final')
                        or (params['phase'] == 'after') != isinstance(params['operation'], dict)
                        or (params['phase'] != 'after' and params['operation'] is not None)):
                    raise Stop('invalid_method_params')
                value = asdict(await adapter.evaluate(params['phase'], params['operation']))
            elif method == 'select':
                if provider is None or not isinstance(params, dict) or set(params) != {
                        'version', 'observation', 'candidates', 'remaining_inputs'}:
                    raise Stop('provider_unavailable_or_request')
                value = await provider.select(params)
            elif method == 'close':
                value = await adapter.close()
                closed = True
            else:
                raise Stop('unknown_method')
            response['result'] = value
        except asyncio.CancelledError:
            response['error'] = 'cancelled'
        except Stop as exc:
            code = str(exc)
            response['error'] = code if 0 < len(code) <= 80 and all(
                c in 'abcdefghijklmnopqrstuvwxyz0123456789_' for c in code) else 'adapter_error'
        except Exception:
            response['error'] = 'adapter_error'  # No arbitrary exception text.
        finally:
            if method == 'select' and provider is not None:
                drain = getattr(provider, 'take_evidence', None)
                rows = drain() if drain else []
                if not isinstance(rows, list) or len(rows) > 1 or len(encoded(rows)) > 131072:
                    response = {'version': 1, 'id': message['id'], 'error': 'provider_evidence_size', 'receipts': []}
                else:
                    response['receipts'] = rows
        return response

    async def send(response):
        raw = encoded(response) + b'\n'
        if len(raw) > MAX_RESPONSE:
            raw = encoded({'version': 1, 'id': response['id'], 'error': 'protocol_reply_size', 'receipts': []}) + b'\n'
        writer.write(raw)
        await writer.drain()

    try:
        incoming = asyncio.create_task(reader.readline())
        while True:
            done, _ = await asyncio.wait([incoming] + ([active] if active else []),
                                         return_when=asyncio.FIRST_COMPLETED)
            if active in done:
                response = active.result()
                last_id, active = active_id, None
                await send(response)
                if closed:
                    break
            if incoming in done:
                raw = incoming.result()
                if not raw:
                    break
                message = _decode(raw, MAX_REQUEST)
                if (set(message) != {'version', 'id', 'method', 'params'} or message['version'] != 1
                        or type(message['id']) is not int or not isinstance(message['method'], str)):
                    raise ValueError('invalid_method_envelope')
                if message['method'] == 'cancel':
                    if message['params'] is not None:
                        raise ValueError('invalid_cancel')
                    if active and message['id'] == active_id:
                        active.cancel()
                        response = await active
                        last_id, active = active_id, None
                        await send(response)
                    elif active or message['id'] != last_id:
                        raise ValueError('invalid_cancel')
                    # A completed method already sent its single response.
                else:
                    if active or message['id'] != last_id + 1:
                        raise ValueError('queued_or_stale_method')
                    active_id = message['id']
                    active = asyncio.create_task(invoke(message))
                incoming = asyncio.create_task(reader.readline())
    finally:
        for task in (incoming, active):
            if task and not task.done():
                task.cancel()
        await asyncio.gather(*(t for t in (incoming, active) if t), return_exceptions=True)
        if not closed:
            await asyncio.wait_for(adapter.close(), 10)


async def serve_stdio(adapter, *, provider=None):
    """POSIX adapter worker. Its owning controller handles SIGINT cancellation."""
    previous = signal.signal(signal.SIGINT, signal.SIG_IGN)
    loop = asyncio.get_running_loop()
    reader = asyncio.StreamReader(limit=MAX_REQUEST)
    incoming, _ = await loop.connect_read_pipe(lambda: asyncio.StreamReaderProtocol(reader), sys.stdin.buffer)
    outgoing = None
    try:
        outgoing, protocol = await loop.connect_write_pipe(
            lambda: asyncio.streams.FlowControlMixin(loop=loop), sys.stdout.buffer)
        writer = asyncio.StreamWriter(outgoing, protocol, None, loop)
        await serve(adapter, reader, writer, provider=provider)
    finally:
        incoming.close()
        if outgoing:
            outgoing.close()
        signal.signal(signal.SIGINT, previous)
