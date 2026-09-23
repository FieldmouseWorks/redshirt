# Bounded diagnostic context comparison

`redshirt-context` compares deterministic evidence retrieval with Jev relevance
scoring before the same diagnostic model in both arms. Version 1 uses the
consumer's ordering and Jev Choice diagnostics. Version 2 uses a Rust BM25
baseline and an optional fixed local Codex CLI for closed-choice diagnostics.
This read-only experiment leaves production context policy unchanged.

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

New campaign preflight also checks the consumer's declared essential evidence
packet. For each case it packs all declared essential IDs in corpus order with
the task and mandatory material, then measures that state's canonical encoded
bytes. A case with an expected diagnosis other than `insufficient` must fit both
`selected_chunks` and `context_bytes`. Preflight rows report
`declared_essential_count`, `declared_essential_context_bytes`,
`declared_essential_feasible`, and `expected_insufficient_control`. An explicitly
expected `insufficient` diagnosis is a frozen abstention control: preflight admits
it even when the declared packet is infeasible and reports that fact. The check
does not alter selection or grading, and the oracle still never reaches a
provider. Saved version-1 and version-2 evidence replays under its original
validation rules.

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
The structural admission check cannot detect dependencies omitted from the
consumer's declaration or establish semantic sufficiency. Corpus completeness
remains with [Conary #1060](https://github.com/FieldmouseWorks/Conary/issues/1060).

## Version 2: lexical retrieval and a fixed coding model

The follow-up contract is owned by [#24](https://github.com/FieldmouseWorks/redshirt/issues/24),
driven by [Conary #1057](https://github.com/FieldmouseWorks/Conary/issues/1057).
Use fresh consumer cases; do not reuse the first pilot as a tuning set. Version 2
adds `baseline: "bm25_v1"` and a frozen `diagnostic` profile to the manifest.
The baseline tokenizes task text and each excerpt's path/content, preserving
whole identifiers and splitting snake/camel identifiers. It uses BM25 with
`k1=1.2`, `b=0.75`, unique query terms and positive IDF. Choices and answer labels
do not enter ranking. Ties retain the consumer's path/owner order. Treatment
score ties retain this stronger baseline order. Selection still changes only
which whole excerpts fit the common limits.

Both diagnostic arms request the profile's model at low reasoning effort.
The profile freezes CLI version `codex-cli 0.154.0`, executable SHA-256, and a
single-model catalog with execution/code-mode metadata disabled. Freeze a
sanitized copy of the local bundled catalog: no `tool_mode`, null
`apply_patch_tool_type` and `model_messages`, empty `experimental_supported_tools`,
`node_repl_disabled: true`, and task-only `base_instructions`. The model alias is
requested consistently; it is not proof of an immutable server-side snapshot.

Live version-2 execution additionally requires
`--codex-executable /absolute/path/to/codex`. Rust verifies its executable hash,
starts it in a private temporary directory, supplies only the diagnostic packet
on stdin, and removes the directory after exit. User config and execution rules
are ignored, project file discovery is disabled, and an environment allowlist
excludes Jev/API credentials. Existing Codex authentication remains available.
Shell, file, browser, connector, plugin, memory, and delegation tools are disabled.
The CLI may advertise its unavailable-in-exec user-input tool; any invocation,
unexpected event, extra answer, invalid choice or missing usage invalidates the
transcript and stops the campaign. One completed turn and one JSON answer are
required. Output is bounded; cancellation/deadline kills and reaps the child.

Each case reserves one Jev selector call and two CLI diagnostic turns. At most
four Jev calls and eight CLI turns fit the existing twelve-stage ceiling. Each
CLI turn has a 60-second deadline, with request and stream retries configured to
zero. The caller never retries or continues after failure. CLI-internal HTTP
attempt counts are not exposed, so recorded turns must not be called exact HTTP
request counts. CLI stdout/stderr, stdin packet, profile hash, usage and exit
status are retained. Receipts bind the exact supplied packet, not the CLI's
internal HTTP envelope or ambient global instructions. Verify the local wrapper
against a fake transport and record common ambient-context identity before live
collection; raw host captures remain local.

The USD reservation applies only to Jev. Codex subscription billing remains
unknown; input, cached-input and output tokens are reported separately. The
combined dollar estimate is null. No dollar cost ratio or cache savings should
be inferred from turn counts. Alternating arm order reduces a fixed ordering
bias but does not remove startup, caching or model stochasticity. Version-1
evidence remains replayable without invoking either provider.

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
