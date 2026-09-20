# Bounded Rust provider comparisons

`redshirt-compare` runs the existing controller with a consumer's deterministic
baseline and optional native Jev provider. The consumer owns fixed cases, the
baseline's interpretation of visible facts, and independent task checks. Rust
owns campaign admission, calls, reservations, timing, evidence and analysis.

Build with `cargo build --locked`. The default mode exercises Jev's batch parser
using manufactured local responses; it makes no network call and reads no key:

```sh
target/debug/redshirt-compare --manifest /path/to/cases.json --output /path/to/new-proof
```

After a successful consumer preflight and a fresh bounded allowance, build with
`cargo build --locked --features jev-http` and add `--live`. Only that mode reads
`TYPESAFE_API_KEY`. The trusted adapter process has that variable removed from its
environment. This exclusion is not a process sandbox. Both modes use fresh output
directories; existing campaigns cannot be resumed or overwritten. A new directory
does not authorize another live allowance.

## Manifest and admission

The manifest is strict version-1 JSON, at most 64 KiB. It contains:

- `cases`: two to eight unique IDs, each with `split` (`calibration` or `held_out`)
  and host-selected `adapter` argv. Both splits must exist before collection.
- `limits`: the existing episode limits, identical for both arms.
- `jev`: the [native provider configuration](JEV-RUST.md). Its request limit must
  equal the episode request limit, and its confidence floor must be disabled.
- `max_live_calls` and `max_reserved_usd`: the entire campaign's explicit allowance.
  Cases times per-case calls must fit before any process starts. Hard limits are
  96 calls and $1, further restricted by the supplied allowance.
- `thresholds`: a strictly increasing list of at most 11 probability cutoffs,
  beginning at zero, fixed before collection.

The provider's pinned price and maximum input reservation determine conservative
admission. A full case is reserved and checkpointed before any possible dispatch.
Interrupted cases retain their reservation. There are no retries or budget refunds.
The checkpoint is local evidence, not an account-wide billing service. Usage-based
estimates are not invoices; failed/interrupted calls can have unknown usage.

Each adapter worker serves the existing method protocol plus a deterministic
provider whose `select` consumes only the same request a model sees. All baseline
episodes run first. Missing or malformed task checks stop the campaign before the
model arm. The consumer preflight must also establish that the scenarios and
baseline meet the intended acceptance criteria; campaign completion alone does
not establish task success.

For the candidate arm, the descriptor identity and entire initial request digest
must match the baseline. The request comparison occurs before provider dispatch.
Later states can diverge after different decisions or real-time world progression;
the ordinary freshness and independent-effect checks still apply. The runner
preserves complete offered menus and does not equalize decisions after divergence.

## Independent measurements

The consumer's verdict adds this reserved namespace to `checks`:

```json
{"comparison":{"complete":false,"useful_action":null}}
```

`complete` is an independent task predicate, separate from `Verdict.ok` (invariant
and effect correctness). Set `useful_action` to a Boolean after each checked
operation and null at reset/final. The consumer must document what constitutes
useful progress. The model's advisory answer cannot populate either field.

The report separates verified task completion, checked operations without useful
task progress, attempts without grades, stop reasons, per-selection and total
episode duration, provider calls, estimated cost, conservative reservations and
unknown usage. A clean early stop can still fail the task. A selected action that
was refused before execution remains ungraded. Rejected or ineffective executed
actions can receive an independent negative grade.

`comparison.json` is checkpointed after every completed episode, with the manifest
digest, identity, initial request digest and measurement. Each episode retains the
existing `report.json`, `events.jsonl`, artifact hashes and concrete `replay.json`.
Replay uses the ordinary `redshirt --replay` path with zero provider calls.
Private consumer manifests and evidence stay private.

## Confidence analysis limits

The tables report accepted mistakes, abstentions, useful decisions lost to
abstention and ungraded decisions separately for calibration and held-out cases.
Stop choices are graded from the independent final task check. Unknown effects or
confidence remain ungraded; they do not become correct decisions by omission.

These are counts on observed decisions collected without a confidence cutoff.
After an abstention, later states would change, so the tables cannot establish
counterfactual episode completion. They select no production threshold and make
no claim that confidence is calibrated. Local mock probabilities and auxiliary
answers are manufactured and provide only a protocol check.

## Verification

`cargo test --locked --test comparison` covers whole-campaign admission, missing
and malformed task labels, clean early stops, bad final checks, initial mismatch
before provider use, cancelled selection timing, split isolation, native mocked
Choice/Score/Noul batches, real process adapters and concrete model-free replay.
Existing controller/provider tests retain stale state, failed effects, cancelled
calls and mandatory finalization coverage. Run default and native-feature suites,
fmt and clippy, plus the consumer's actual game/application checks.
