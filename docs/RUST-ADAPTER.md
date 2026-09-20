# Rust controller and trusted adapter protocol

Rust owns admission, budgets, candidate binding, cancellation, evidence and
concrete replay. The adapter process implements environment methods; it never
runs the Python controller underneath Rust. The Python runner and selector-facing
JSON API remain the transition baseline for callers not yet migrated. Conary's
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
listener or model credential lookup is implemented by the executable.

Choose exactly one of `--script PATH`, `--remote-provider`, `--replay PATH`.
`--limits PATH` reads a bounded JSON Limits object. Rust callers use Adapter and
Provider traits, signal the cancellation token, and await finalization rather
than aborting the entire run future. The executable maps SIGINT to cancellation.

## Method boundary

`redshirt.adapter_stdio.serve_stdio(adapter, provider=...)` hosts Python adapters.
Each JSON-lines request has exactly `version:1`, increasing `id`, `method` and
`params`. Replies contain `version`, matching `id`, `receipts`, and exactly one
of `result` or `error`. Errors are bounded classification codes; arbitrary
exception text and stderr are not captured. Duplicate keys are rejected at every
depth. Requests cap at32KiB, responses at192KiB, with tighter field limits.

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

The first cross-language profile accepts views with integer JSON numbers
representable by serde_json, sorted object keys and Python-compatible ASCII
escapes. Floating-point view numbers refuse as nonportable_view_number instead
of relying on unproven cross-runtime float canonicalization. Receipt/report
timings may still be fractional. This covers the demonstrated adapter; general
numeric canonicalization, captures and migration of interactive callers remain
follow-up work.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
REDSHIRT_BIN="$PWD/target/debug/redshirt" python3 -m unittest discover -s tests -v
```

Synthetic controls cover refusal, budgets, negative effects, uncertainty,
attachment, strict JSON and hashes. Real pipes exercise timeout/SIGINT, cancelled
provider receipts, worker exit and Python/Rust replay in both directions. Private
integration adds ordinary browser inputs, game authority and observation
filtering. These establish implementation parity, not model advantage or
historical fidelity. Live usage remains separately authorized and bounded.
