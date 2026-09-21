# Rust controller and trusted adapter protocol

Rust owns admission, budgets, candidate binding, cancellation, evidence and
concrete replay. The adapter process implements environment methods; it never
runs the Python controller underneath Rust. The selector-facing JSON API now also
uses Rust through `--stdio`, with the unchanged Python client. The Python runner
remains the explicit transition baseline for other callers. Conary's
existing Rust pilot is unchanged.

## Synthetic example

Rust1.98.0 and Python3.11+ are tested. Cargo.lock pins dependency resolution.
The Python bridge has no third-party dependencies.

```sh
cargo build --locked
printf '["increment","stop"]\n' > /tmp/redshirt-choices.json
target/debug/redshirt --output /tmp/new-redshirt-run \
  --script /tmp/redshirt-choices.json \
  --adapter python3 tests/rust_fixture.py normal
target/debug/redshirt --output /tmp/new-redshirt-replay \
  --replay /tmp/new-redshirt-run/replay.json \
  --adapter python3 tests/rust_fixture.py normal
```

Output directories must be new; Unix directories use mode0700. Each bundle has
events, report, concrete replay and SHA-256 manifest. The executable prints a
small terminal summary. Argv, paths, scripts, limits and provider configuration
are host configuration, never selector output. No shell interpretation, network
listener is implemented by the executable. Only explicit `--jev CONFIG` mode with
the optional `jev-http` feature looks up a model credential; see [Jev](JEV-RUST.md).

Choose exactly one of `--script PATH`, `--remote-provider`, `--replay PATH`,
`--stdio`, or native `--jev CONFIG`. `--limits PATH` or `--limits-json JSON` reads
a bounded JSON Limits
object (at most4096 bytes; mutually exclusive). Rust callers use Adapter and
Provider traits, signal the cancellation token, and await finalization rather
than aborting the entire run future. The executable maps SIGINT to cancellation.

## Method boundary

`redshirt.adapter_stdio.serve_stdio(adapter, provider=...)` hosts Python adapters.
Each JSON-lines request has exactly `version:1`, increasing `id`, `method` and
`params`. Replies contain `version`, matching `id`, `receipts`, and exactly one
of `result` or `error`. Errors are bounded classification codes; arbitrary
exception text and stderr are not captured. Duplicate keys are rejected at every
depth. Requests cap at64KiB, responses at384KiB, with tighter field limits.

| Method | Parameters | Result |
|---|---|---|
| describe | null | identity and setup_mode (reset or attach) |
| reset | null | null after declared setup |
| observe | null | observation and code-generated candidates |
| verify | null | verified current descriptor |
| execute | exact offered operation | input receipt, separate from correctness |
| evaluate | phase (reset/after/final) and operation or null | independent ok/progress/checks |
| close | null | null after releasing resources |
| select | authorized decision request | one candidate ID; configured provider only |

Only `select` receives provider input: version, authorized observation, candidate
ID/description map and remaining input units. Guards, executable payloads and
evaluator checks stay outside that request. Role policy and information filtering
belong to adapters. This trusted local transport is not an OS sandbox or remote
authentication boundary.

Only one method may run. After a dropped/timed-out RPC, the controller sends
`cancel` with its ID and null params. The bridge cancels and joins the method,
returning one response; cancellation crossing a completed response is harmless.
Rust drains it, including provider receipts, before final evaluation. Partial
frames remain buffered across cancellation. Inputs are never retried. Protocol
corruption makes checks/cleanup fail explicitly; the executable kills and reaps
its direct child if graceful close fails. Cleanup of uncooperative descendants
is not guaranteed by this process boundary.

## Limits and replay

Ceilings:24 attempted input units,24 decisions,180 seconds,10 seconds/operation,
separate10-second final-check/cleanup reserves,8MiB evidence and three consecutive
nonprogress actions. This first Rust path requires captures=0. Intent/accounting
precede uncertain effects. Before input, Rust verifies identity, exact observation
and candidates, readiness, cancellation and remaining budget. Final evaluation,
identity recheck and close run even after Stop/refusal/cancellation.

Replay requires verified reset, matching identity, uniquely available concrete
operations, matching view digests and independently checked outcomes. Attached
sessions never grant reset/replay authority. Uncertain inputs remain reported
and cannot produce a complete replay. The v1 replay shape is retained.

The v1 cross-language profile accepts serde_json's integer range and finite
binary64 numbers, sorted object keys and Python-compatible ASCII escapes. A
custom formatter preserves shortest round-trip float digits, signed zero and
Python JSON's exponent notation. Parsing uses float_roundtrip. An independent
Python JSON oracle checks edge cases and a seeded16,384-pattern float corpus;
fractional synthetic views also replay under Python. This supersedes the initial
integer-only restriction without changing existing integer trace hashes.
It is not arbitrary-precision numeric canonicalization. Actual clock changes
remain changes: a privileged time-bearing view can fail exact replay, and must
not be rounded or normalized to manufacture agreement. Captures remain unavailable.

## Host-configured menu bounds

Existing configurations retain 96 adapter candidates, 16 KiB of encoded candidate
records and 16 KiB per decision request. Trusted hosts can set `candidates`
(1–254), `candidate_bytes` (1–64 KiB) and `decision_bytes` (1–32 KiB) in Limits.
Stop is added separately, keeping the existing Choice maximum of 255 options.
The controller validates the entire menu initially and again before execution;
no provider/client reply can change these settings. Over-limit menus are refused,
never pruned. Observation views remain capped at 4 KiB, and all input, call,
time, cancellation, freshness and total evidence limits remain in force.

A wider controller menu can still exceed a separately configured provider bound.
The [optional providers](JEV-RUST.md) accept an explicit `request_bytes` setting;
16 KiB remains the default, with a 64 KiB ceiling for the complete encoded body.
Responses still cap at 16 KiB. Expanded providers reserve up to 256 KiB of receipt
space before dispatch. The process-provider boundary conservatively reserves
256 KiB for any remote receipt, so a small remaining evidence allowance can now
stop earlier; it never increases the episode's total evidence limit.

Local method and [decision transports](INTERACTION.md) have bounded envelopes
large enough for admitted requests, complete action schemas and receipts. Legacy
wire/replay versions and existing Python clients remain supported. Consumers
must use the matching updated bridge/client package and executable when opting
into wider menus. These synthetic checks establish contract behavior, not a
provider's ability to choose well from a large menu.

## External decisions

`--stdio` implements the existing [decision protocol](INTERACTION.md), separate
from the trusted adapter method pipe. It requires Unix pipe stdin/stdout and
refuses regular files or terminals; use the unchanged InteractionClient to create
those pipes. Only observation/tool and done envelopes appear on stdout. The
adapter worker runs beneath Rust; there is no nested Python controller.

Decision tokens use16 OS-random bytes. Replies have exactly decision_id/action_id;
duplicate keys, malformed/oversized frames, stale tokens and unavailable choices
stop without retry. Decision receipts survive dropped futures. Partial outgoing
frames retain their write offsets across cancellation. Unix asynchronous pipes
allow timeout/SIGINT finalization even while the client's stdin remains open.
After checks and child cleanup, sending the terminal envelope gets at most five
seconds; missing delivery does not erase the report or change the game verdict.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo test --locked --all-features
cargo build --locked
REDSHIRT_BIN="$PWD/target/debug/redshirt" python3 -m unittest discover -s tests -v
```

Synthetic controls cover refusal, budgets, negative effects, uncertainty,
attachment, strict JSON and hashes. Real pipes exercise timeout/SIGINT, cancelled
provider receipts, worker exit and Python/Rust replay in both directions. Private
integration adds ordinary browser inputs, game authority and observation
filtering. These establish implementation parity, not model advantage or
historical fidelity. Live usage remains separately authorized and bounded.
