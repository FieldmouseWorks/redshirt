"""Subprocess transport fixture: synthetic counter only."""
import asyncio
from pathlib import Path
import signal
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from redshirt.interaction import serve_stdio
from test_runner import Fixture


async def main():
    cancel = asyncio.Event()
    interrupted = asyncio.Event()
    def on_sigint():
        interrupted.set()
        cancel.set()
    asyncio.get_running_loop().add_signal_handler(signal.SIGINT, on_sigint)
    await serve_stdio(Fixture(), Path(sys.argv[1]), cancel=cancel)
    if len(sys.argv) > 2 and sys.argv[2] == 'terminal-exit-seven':
        # The final frame is already on the wire. A close() signal here would
        # change the child's natural exit status despite completed checks.
        await asyncio.sleep(.2)
        return 42 if interrupted.is_set() else 7
    return 0


if __name__ == '__main__':
    raise SystemExit(asyncio.run(main()))
