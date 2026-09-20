# Local structured interaction

`redshirt.interaction` connects any external decision system to the existing
runner through JSON lines on local pipes. A scripted client, an LLM tool handler
or a decision model sees the same adapter-audited observation and choices. No
image, network server, model SDK or credential is required by this transport.

An adapter CLI can expose an episode with:

```python
from redshirt.interaction import serve_stdio

report = await serve_stdio(adapter, fresh_output_directory, cancel=cancel_event)
```

The caller creates the adapter, chooses its permissions and handles SIGINT by
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
Outbound messages are at most32KiB; replies at most4KiB. There is one pending
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
concurrent actions, and cancels its owned process on close. A failed receive
invalidates the held frame; it does not retry an uncertain action. The transport
uses POSIX pipes; Windows support has not been verified.

Roles, actor assignment and which facts/actions each role may access belong to
the project adapter. Role assignment must happen at its trusted host boundary,
not in a prompt or action argument. The transport forwards only that adapter's
authorized view/menu. Account authentication and remote authorization are not
provided by a local pipe connection.

Tests use a synthetic counter, real subprocess pipes, malformed/stale replies,
disconnect/cancellation, a missed-input negative control and concrete replay.
They contain no private project assets or live model requests.
