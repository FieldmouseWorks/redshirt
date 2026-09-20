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
    asyncio.get_running_loop().add_signal_handler(signal.SIGINT, cancel.set)
    await serve_stdio(Fixture(), Path(sys.argv[1]), cancel=cancel)


if __name__ == '__main__':
    asyncio.run(main())
