# Bounded diagnostic context comparison

`redshirt-context` compares a consumer's deterministic evidence ordering with
Jev relevance scoring before the same pinned Jev diagnostic Choice. This is a
read-only classification experiment. It does not run commands, change production
context policy, or establish usefulness for a larger coding model.

Consumers own source-pinned cases, actual owner packets, baseline ordering,
complete diagnosis choices, and a separate independent answer/evidence key.
Rust owns request construction, selection limits, call reservations, evidence,
analysis, and offline replay. The first consumer is
[Conary #1055](https://github.com/FieldmouseWorks/Conary/issues/1055); shared
implementation and exact proof are tracked in [#22](https://github.com/FieldmouseWorks/redshirt/issues/22).

## Frozen protocol

Supply a version-1 `manifest.json` and separate `oracle.json`. The latter binds
the canonical manifest SHA-256 and maps each case to its expected diagnosis,
essential evidence IDs, and independent proof commands. It never reaches the
provider. Required project instructions and each case's mandatory owner packet
are retained in every request. Both diagnosis arms use identical tasks,
instructions, complete Choice options, model version, and context limits.

The baseline greedily packs whole chunks in the consumer's frozen order. The
treatment scores every optional chunk in one batch against the same task, then
packs by descending relevance; ties preserve baseline order. Both obey the same
chunk and encoded-context byte ceilings. The selector cannot remove mandatory
material or change permissions, options, or grading. Context order follows the
original corpus order in both arms; scores change inclusion only.

Each case uses one baseline diagnostic call, one selection call, and one
treatment diagnostic call. Even-index cases run the baseline first; odd-index
cases run selection/treatment first. Freeze case order, splits, prompts,
selection policy and hashes before collection. No response-dependent tuning,
replacement cases, retries, fallback, or continuation after a failed call.

Bounds are two to four cases, at most eight optional chunks per case, up to four
selected chunks, 30,000 encoded context bytes, and 32 KiB request bodies. A
campaign admits at most 12 calls and $0.04 of conservative input reservations.
The entire worst-case allowance must fit before dispatch. Each possible call is
checkpointed before dispatch and retains its reservation on interruption.
Existing output directories are refused; a new directory does not renew a live
allowance. The existing five-second provider deadline and 16 KiB response cap
apply. No real provider is used by default.

## Commands

```sh
cargo build --locked --bin redshirt-context
target/debug/redshirt-context --manifest /path/to/manifest.json \
  --oracle /path/to/oracle.json --preflight
target/debug/redshirt-context --manifest /path/to/manifest.json \
  --oracle /path/to/oracle.json --output /path/to/new-mock-proof
target/debug/redshirt-context --replay /path/to/new-mock-proof
```

Mock answers are manufactured independently of the grading key; they verify the
protocol and carry `quality_evidence: false`. After consumer preflight, an
explicit bounded live allowance, and secure `TYPESAFE_API_KEY` provisioning,
build with `--features jev-http` and add `--live` to a new campaign. Both arms
and the selector use `jev-1.13.0`; no additional provider credential is needed.

## Evidence and interpretation

The fresh output directory contains the frozen manifest, independent oracle,
one call record per attempted stage, and a checkpointed report. Receipts retain
exact requests, response text, hashes, usage, validation and failed/interrupted
outcomes. Replay validates request identity, reconstructs selected packets and
independent grades, and compares the recorded analysis with zero provider calls.
Hashes establish consistency with the saved inputs, not cryptographic authorship.

Reports separate calibration and held-out accuracy, insufficient-evidence
answers, ungraded cases, missing essential chunks, context bytes, selection and
diagnostic latency, and calls. Treatment cost and provider latency include its
selection stage, including costs when a later diagnostic call fails. Campaign
wall time also includes local processing and evidence writes. Jev exposes no
cache-usage measurement here; no cache-rebuild savings are claimed. Reported
input-token estimates are separate from conservative reservations and unknown
billing. Unknown usage after a failed/interrupted call remains explicit.

The corpus is small, hand-selected, and multiple-choice. Essential-chunk labels
measure the consumer's declared evidence criterion, not whether the model relied
on those chunks. A correct diagnosis may be inferable from the task or mandatory
policy even when labelled evidence is absent. A result cannot establish general
coding-agent accuracy, confidence calibration, or a production routing policy.

## Verification

```sh
cargo test --locked --test context
cargo test --locked --test jev
cargo test --locked --all-features
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo fmt --all -- --check
```

Injected tests cover label isolation, identical required policy, changed grading
keys, malformed selector answers, cancellation, reservations, byte limits,
tampered requests/results, and model-free replay. Existing action-provider tests
remain required because both interfaces share the same transport and receipts.
