# Local structured interaction

The Rust executable's `--stdio` mode connects external decision systems to the
controller through JSON lines on local pipes. The existing Python
`redshirt.interaction.InteractionClient` preserves this wire contract. A scripted client, an LLM tool handler
or a decision model sees the same adapter-audited observation and choices. No
image, network server, model SDK or credential is required by this transport.

Build with `cargo build --locked`, then supply trusted adapter arguments through
the [method boundary](RUST-ADAPTER.md). For example:

```python
from redshirt.interaction import InteractionClient

async with await InteractionClient.start(
    "/path/to/redshirt", "--stdio", "--output", "/tmp/new-episode",
    "--adapter", "python3", "tests/rust_fixture.py", "normal"
) as client:
    await client.act("increment")
    done = await client.act("stop")
```

The legacy Python controller remains an explicit migration baseline. An adapter
CLI can still select it with:

```python
from redshirt.interaction import serve_stdio

report = await serve_stdio(adapter, fresh_output_directory, cancel=cancel_event)
```

For that legacy entrypoint, the caller creates the adapter, chooses its permissions and handles SIGINT by
setting the cancellation event. Stdout belongs exclusively to the protocol.
Diagnostics belong on stderr; full reports, checks and replay remain in the
private evidence directory. Capture limit defaults to zero for this entry point.

## Protocol

The process first sends one `observation` line:

```json
{"version":1,"type":"observation","decision_id":"opaque-token","observation":{"counter":0},"candidates":{"increment":"Press the increment control.","stop":"Stop the experiment."},"remaining_inputs":24,"tool":{"name":"choose_action","description":"Choose one currently offered action.","parameters":{"type":"object","properties":{"action_id":{"type":"string","enum":["increment","stop"]}},"required":["action_id"],"additionalProperties":false}}}
```

The client sends exactly one response line:

```json
{"decision_id":"opaque-token","action_id":"increment"}
```

The next observation arrives after execution and its independent check. A valid
game refusal may still pass that check; read the new observation for the actual
effect. Choosing an action is not an acknowledgment that it succeeded. On stop
or failure, a terminal envelope follows mandatory checks and cleanup:

```json
{"version":1,"type":"done","stop":"selector_stop","requests":2,"attempted_inputs":1,"verified":true,"cleanup":true,"replayable":true}
```

`verified` describes the final independent verdict, not the reason for stopping.
For example, a rejected malformed request can leave the environment intact.
The wire never includes the evaluator's private checks, raw operations or paths.

Tokens bind one reply to one pending decision and change every time. A stale or
prequeued duplicate, unknown action, extra field, malformed/oversized JSON or EOF
stops the episode without retry. The runner revalidates the environment and
candidate immediately before execution. It retains all existing budgets,
cancellation, no-progress limits, evidence and provider-free concrete replay.
Outbound messages are at most64KiB; replies at most4KiB. Host-configured
[menu limits](RUST-ADAPTER.md#host-configured-menu-bounds) retain the old defaults. There is one pending
decision. The runner's operation deadline includes the client's thinking time
(at most10 seconds); real-time environments continue advancing during that wait.

## Async client

The command and its permissions are operator configuration, never model output.
The child must be an adapter CLI implementing the protocol above.

```python
from redshirt.interaction import InteractionClient

async with await InteractionClient.start(*trusted_adapter_argv) as client:
    frame = client.observe()
    while frame["type"] == "observation":
        # Your provider/tool handler receives observation, candidates and tool.
        chosen_id = await choose(frame["observation"], frame["candidates"], frame["tool"])
        frame = await client.act(chosen_id)
```

The client inserts the current token, returns detached observations, rejects
concurrent actions, and owns process shutdown. After a valid terminal `done` frame,
`close()` waits for normal exit instead of sending cancellation, preserving the
actual process status. An unfinished session still receives SIGINT. Waiting is
bounded by the existing 30-second timeout, followed by kill and wait if needed.
The terminal frame describes episode checks; inspect `client.process.returncode`
after close for the separate process result. A failed receive invalidates the
held frame; it does not retry an uncertain action. The transport uses POSIX pipes;
Windows support has not been verified.

Leave status collection to asyncio's process watcher. Consumers must not call a
second `waitpid`, `Popen.poll` or `Popen.wait` on the same child. [Issue #19](https://github.com/FieldmouseWorks/redshirt/issues/19)
reproduces an older asyncio signal path that could reap an already-completed
child before the watcher, yielding an unknown-status warning and synthetic 255.
Graceful waiting after `done` avoids that demonstrated path. This does not claim
to repair unrelated interpreter startup, forced-kill or external-reaper races.

Roles, actor assignment and which facts/actions each role may access belong to
the project adapter. Role assignment must happen at its trusted host boundary,
not in a prompt or action argument. The transport forwards only that adapter's
authorized view/menu. Account authentication and remote authorization are not
provided by a local pipe connection.

Tests use a synthetic counter, real subprocess pipes, malformed/stale replies,
disconnect/cancellation, a missed-input negative control and concrete replay.
They contain no private project assets or live model requests.
