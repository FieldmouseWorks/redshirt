# Fixed JSON candidate selectors

`--choice CONFIG` is an optional native Rust provider under the existing
controller. It selects one offered candidate ID. The adapter still owns the
observation, actions and independent checks; concrete replay uses no provider.
No controller, campaign logic, tool access or model-generated operation is added.

## Config and CLI

Build with `--features choice-http`. The strict version-1 config is:

```json
{"version":1,"profile":"deepseek_flash","request_limit":12}
```

`profile` is exactly `openai_luna` or `deepseek_flash`. `request_limit` defaults
to six and must be 1–12 and no greater than controller `Limits.requests`.
`request_bytes` defaults to 16,384 and may be explicitly reduced to 1,024–16,384.
No other fields are accepted. The CLI loads at most 4 KiB of config before
starting an adapter. For example:

```sh
target/debug/redshirt \
  --output /path/to/new-run --choice /path/to/choice.json \
  --expected-initial 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --limits /path/to/limits.json --adapter python3 /path/to/adapter.py
```

Provision `OPENAI_API_KEY` for `openai_luna`, or `DEEPSEEK_API_KEY` for
`deepseek_flash`, in the invoking environment. The executable strips those and
`TYPESAFE_API_KEY` from the adapter child environment in every mode. This is a
credential-exclusion measure, not an OS sandbox. No provider key belongs in
config, command arguments, output or adapter input. Without `choice-http`, the
mode refuses before adapter setup. `--choice` is exclusive with script, replay,
remote provider, stdio and Jev. Default execution and replay remain model-free.

`--expected-initial SHA256` works with `--choice`, `--jev` and
`--remote-provider`; other modes reject it. It requires 64 lowercase hex digits.
The existing `MatchedProvider` hashes the **first complete controller request**
with `evidence::encoded` and compares it before provider dispatch. A mismatch
stops with `comparison_initial_mismatch`, no model call and mandatory final
checks/cleanup. Obtain the reference digest from a fresh model-free case's first
`request` event, not by hashing an observation alone.

Each provider-mode output has `selection.json` with version, provider name,
expected and observed initial digests, and ordered `elapsed_ms` values. Timings
cover each full selection future, including calls interrupted by cancellation or
controller timeout. They pair in order with provider receipts. A pre-dispatch
initial mismatch has no elapsed entry. This file is separate from the controller's
`artifacts.json`; hash it separately when archiving evidence. Output directories
must be new.

## Fixed wire profile

Both profiles POST only to their official HTTPS Chat Completions endpoint, with
TLS verification and no inherited proxy, redirect, retry, decompression, tools,
fallback, previous response or conversation history. Each call sends the
canonical serialized current controller request as one user message. The system
message repeats Jev's action-objective wording verbatim and adds only the JSON
format instruction: `{"action":"candidate_id"}`. The returned ID must exactly
match an offered candidate, including `stop`; output with extra keys, duplicate
keys, refusal or incomplete `finish_reason` fails closed.

| Profile | Endpoint/model | Fixed request fields beyond messages |
| --- | --- | --- |
| `openai_luna` | `https://api.openai.com/v1/chat/completions`, `gpt-6-luna` | `reasoning_effort:"none"`, `max_completion_tokens:64`, `response_format:{"type":"json_object"}`, `service_tier:"default"`, `store:false` |
| `deepseek_flash` | `https://api.deepseek.com/chat/completions`, `deepseek-flash` | `thinking:{"type":"disabled"}`, `max_tokens:64`, `response_format:{"type":"json_object"}` |

The response must report the exact requested model string. This is a response
field check, **not** backend attestation. The DeepSeek `deepseek-flash` API name
is deliberate; the old `deepseek-v4-flash` alias is not requested or accepted.
See the [GPT-6 Luna model page](https://developers.openai.com/api/docs/models/gpt-6-luna),
[DeepSeek Chat API](https://api-docs.deepseek.com/api/create-chat-completion/),
[thinking control](https://api-docs.deepseek.com/guides/thinking_mode/), and
[JSON output guidance](https://api-docs.deepseek.com/guides/json_mode/).

The observation remains capped at 4,096 canonical bytes. The entire API JSON
request is capped by `request_bytes`; a response is capped at 16,384 bytes.
These bounds refuse before dispatch where possible. One call is in flight with
a five-second deadline including body reading. The native transport retains a
bounded response prefix on HTTP oversize. The controller still controls its
episode, operation and total-evidence limits.

## Receipts and economics

Call reservation precedes dispatch. A dropped selection leaves a pending
`interrupted` receipt that the controller drains. Receipts record bounded,
credential-redacted raw request/response, request hash, UTC Unix-millisecond
timestamps, elapsed milliseconds, HTTP status, selected rate-limit headers,
requested/reported model, outcome/error and usage when valid. A malformed supplied
usage object fails; absent usage is unknown. Valid usage survives an invalid
choice. Cache-read, cache-write, cache-miss and reasoning counts remain null when
unreported. `billed_usd` is always unknown here. `estimated_usd` remains null
when cache-write or time-dependent tariff inputs are ambiguous; the consumer may
perform a separately documented cache-aware analysis from raw receipts.

Every call conservatively reserves 65,536 input and 64 output tokens. At the
published Standard Luna rates of $0.10 input, $0.01 cache read, $0.125 cache
write and $0.50 output per million, its worst class reservation is $0.008224.
At DeepSeek Flash's peak $0.30 input miss, $0.006 cache hit and $1.20 output
rates, it is $0.0197376. These are allowance reservations, not actual charges.
DeepSeek's off-peak rates are $0.15, $0.003 and $0.60 respectively. Prices were
checked on 2026-09-23 against the [OpenAI API pricing](https://developers.openai.com/api/docs/pricing)
and [DeepSeek model pricing](https://api-docs.deepseek.com/quick_start/pricing/).

Network-free fake-transport tests exercise both bodies, response validation,
usage and failure receipts, cancellation, digest mismatch and actual adapter
pipes/replay. They establish mechanics only. Neither direct provider was called
as part of this implementation; no model quality or live availability is claimed.
