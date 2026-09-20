# Bounded external browser slice

Primary issue: [#3](https://github.com/FieldmouseWorks/redshirt/issues/3).

The Python runner supplies one controller to an external browser adapter. Python
matches that application's existing Playwright verification lane. The proven
Conary Rust controller remains at `4278a9202f2bc87f58d54547f5c03e37cf14d26f`;
none of its source, tests or prior evidence changed. This is a first standalone
boundary, **not completed extraction or cross-language consolidation**. Migrating
Conary's package-specific request, context and evidence types is outside this
bounded browser slice. No package-state engine was copied into a game adapter.

## Contract

`redshirt.run(adapter, output, provider=...)` accepts a trusted adapter implementing
the small protocol in `runner.py`. The adapter owns `reset`, `observe`, `candidates`,
`verify`, `execute`, `evaluate` and `close`; optional `capture` returns a PNG.
An independent evaluator belongs in that adapter and must read the resulting
environment rather than trusting a receipt or selector. The protocol is a
programming boundary, not an isolation sandbox.

Only `Observation.view`, candidate descriptions and remaining input count are
sent to a provider. Environment/epoch guards, executable operations, raw captures
and evaluator checks are excluded. Candidate IDs are the entire response
vocabulary. Before input, the controller verifies the environment, compares a
fresh observation and candidate set, checks readiness and reserves input budget.
The adapter must check its actual target again immediately before physical input.
Busy input is refused; there is no queued decision or automatic retry.

Default limits are 24 attempted inputs, 24 selection requests, 180 seconds,
8 MiB of evidence and 12 captures. An operation has at most 10 seconds; final
checks and cleanup each reserve a separate 10 seconds. Three consecutive
independently checked no-progress steps stop the run. Intent and attempted-input
accounting precede execution, including uncertain or lost receipts. Async adapter
operations must cooperate with cancellation; these are application-level limits,
not process or operating-system isolation.

`Scripted`, `Seeded` and `MockTransport` are model-free. The mock boundary enforces
one request in flight, response size, duplicate-key rejection and an exact
candidate-ID envelope. It is not an implementation or validation of a particular
model service's API, probabilities, latency or billing.

## Evidence and replay

Every output directory must be new. `events.jsonl` is flushed before input;
`report.json`, `replay.json` and SHA-256 `artifacts.json` are finalized after
mandatory checks and cleanup, including Stop, refusal, cancellation and failure.
Capture names are controller-generated. Payload bounds reserve room for final
evidence. Uncertain execution remains in the report and marks replay incomplete.

Use `redshirt.evidence.load_replay(path)`, then
`run(adapter, fresh_output, replay=saved)`. Replay forbids a provider, verifies
artifact/setup identity, restores and checks the baseline, regenerates matching
concrete candidates, and compares observations and independent verdicts. It
refuses incomplete traces and unavailable preconditions; it never substitutes a
new action. Executable replay requires a resettable adapter. A live application with
no verified reset has observational evidence, not an executable-replay claim.

The [attached-session follow-up](https://github.com/FieldmouseWorks/redshirt/issues/5)
makes that boundary explicit: a trusted adapter declares `setup_mode = 'attach'`.
Its existing `reset()` interface hook initializes and verifies attachment only;
`setup_verified` can be true while `reset_verified` stays false. Recorded steps
remain inspectable but their replay completeness is always false. Replay is
refused before the initialization hook or any input, even when supplied evidence
claims completeness. Mandatory final evaluation and cleanup still run.

The default mode remains `reset`, preserving existing adapters and v1 replay
files. Setup mode is frozen alongside identity for an episode; changing it during
selection refuses input and prevents a replayability claim. The mode is an audited
adapter declaration, not proof that arbitrary third-party reset code is correct.
Three additional synthetic tests cover attached recording/forged replay, changed
capability and invalid configuration (19 tests total).

## Measured result and limits

The public suite contains only synthetic fixtures. Sixteen tests cover reset and
replay identity, lost receipts, misleading receipts, stale/changed targets, busy
refusal, input/request/time/no-progress limits, cancellation, user/death/unknown
stops, hidden-data exclusion, evidence bounds and mock response validation.

The private adapter's separate proof performed three ordinary inputs per episode:
two view interactions and one movement. Scripted and mock traces each replayed
all three inputs with zero selector calls and matching independent checks. A
third episode used ordinary real-time progression. Fourteen focused browser
acceptance checks passed, including a rendered-pixel/observation/options equality
test after hidden actor, loot and map mutations, actual user intervention, and
a deliberately broken timing build detected by the evaluator. The application's
existing browser gate passed 1,025 checks. These are implementation measurements,
not historical fidelity or demonstrated model usefulness.

The observation intentionally exposes only two displayed state categories and
recent own-input history. There is no vision claim, general game bot, live model
comparison, external coordinator, deployment or release. The reference client's
files, settings and captures stay outside this repository. No prior live-model
allowance was reused. A deterministic timing probe currently supplies no reason
to spend model requests; any later comparison needs its own concrete data,
request/time/cost bounds and authorization.

Review is self-review, with independently specified checks; no separate reviewer
or delegated worker participated. See the owning PRs and the
[progress issue](https://github.com/FieldmouseWorks/redshirt/issues/1) for exact
candidate revisions and commands.
