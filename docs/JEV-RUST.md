# Rust Jev batches and explicit abstention

Shared typed questions, answer validation and uncertainty policy live in
`src/decision.rs`; `src/jev.rs` owns bounded provider calls and receipts. The
existing Rust controller remains the sole episode owner. Python callers retain
their explicit [transition baseline](JEV.md) until individually migrated.

Each call sends the complete current action Choice, including stop, plus up to
seven independent caller-configured Choice, Score or Noul questions. All read the
same authorized state. Additional answers remain in the raw receipt; they cannot
provide permissions, commands, success verdicts or future actions. A decision
needing an action's result must wait for fresh state. No candidate pruning is used.

## Configuration and invocation

```json
{
  "version": 1,
  "request_limit": 6,
  "questions": {
    "visible": {"type": "noul", "instructions": "Is the counter visible in the observation?"},
    "urgency": {
      "type": "score",
      "instructions": "How urgent is the objective in the observation?",
      "criteria": ["Routine", "Urgent"]
    }
  },
  "policy": {"min_confidence": null}
}
```

The application owns the questions and any calibrated threshold. Empty config
defaults to version 1, six calls, no additional questions and no confidence floor.
`action` is reserved. This first contract accepts string instructions/descriptions,
Choice maps and Score arrays; structured criteria and optional Noul criteria are
not exposed. Unknown fields and invalid configs refuse before adapter setup.

For an explicitly authorized live adapter session, provision `TYPESAFE_API_KEY`
in the invoking environment and run:

```sh
cargo build --locked --features jev-http
target/debug/redshirt --output /path/to/new-run \
  --jev /path/to/jev.json --adapter python3 /path/to/adapter.py
```

`--jev` is exclusive with existing selector/replay modes. Without `jev-http` it
refuses before starting an adapter. Other modes and concrete replay never look up
a key. Credentials do not belong in config or command-line arguments. Rust library
callers use `jev::Jev::new(transport, config)` for injected transports, or
`jev::Jev::live(key, config)` for native HTTPS.

## Policy and evidence

With a configured `min_confidence` in [0, 1], a lower action confidence ends the
episode with `provider_uncertain` before executing that proposal; equality passes.
The receipt preserves the proposed action, reported confidence, threshold and
`abstained` outcome. This is distinct from choosing stop or receiving malformed
data. Final checks and cleanup still run. A caller can review the result and
choose a separately authorized next step. No automatic retry, substituted action,
fallback model or human notification occurs.

No universal threshold is supplied. Confidence describes the distribution, not
measured task success. Calibrate against the actual task and pinned model;
independent adapter checks retain correctness authority. Auxiliary answers are
advisory and cannot bypass admission or change the action menu.

Bounds: eight total questions, 8 KiB config, 16 KiB responses,
1–12 calls, one in flight, five-second absolute transport/body deadline. Complete
request bodies default to 16 KiB. Trusted `request_bytes` config accepts 1–64 KiB;
expanded allowances reserve 256 KiB of receipt space, with no change to the
controller's total evidence budget. Action Choice supports up to 255 options
including stop; the controller's separate host-configured menu limits still apply.
Oversized requests refuse before dispatch and consume no provider call. Native
HTTPS pins `https://api.typesafe.ai/v1/systemone` and `jev-1.13.0`, verifies TLS,
and disables proxies, redirects, retries and decompression. Every uncertain
dispatch consumes a call and a 65,536-input-token reservation. Rust reserves
receipt space first. Dropped selections retain `interrupted` evidence, which must
be drained before another call. Estimates remain separate from unknown billing.

All answers validate before policy runs: exact IDs/types, pinned model, bounded
integer usage, finite probabilities and maximum Choice selection. Choice
distributions cover every option. Score legends match the rubric; sparse
distributions may omit zero-mass levels. The reported mean must remain within
the rubric and match the weighted distribution within the accepted total tolerance
plus rounding. Noul has a single [0, 1] value and no confidence field. Duplicate
keys, unknown fields or one invalid auxiliary answer refuse the whole batch.
Raw probabilities remain unchanged; the [Conary tolerance/provenance](../THIRD_PARTY_NOTICES.md)
is retained. Receipts preserve raw answers, validation totals and successful usage
through abstention. Arbitrary transport exception text and headers are omitted.
Known reflected credentials are redacted. Diagnostic text expanded by redaction
or invalid UTF-8 is bounded again; redaction and truncation are explicitly flagged.

## Verification and limits

The existing Rust fixture compares scripted and three-question batched runs:
identical operations and independent checks, then concrete replay with zero
provider calls. Tests cover all question types, complete menus, malformed auxiliary
answers, abstention, stale state, omitted effects, cancellation, budgets and native
request construction without network. These establish mechanics, not model
accuracy, calibrated thresholds, latency improvement or completed caller migration.

```sh
cargo test --locked
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo test --locked --all-features
```

Protocol sources inspected September 20, 2026: [API](https://docs.typesafe.ai/api),
[primitives](https://docs.typesafe.ai/primitives),
[confidence](https://docs.typesafe.ai/confidence), and
[model limits/pricing](https://docs.typesafe.ai/models).
