# Redshirt progress

This page records demonstrated behavior, its limits and the next bounded slice.
The [progress thread](https://github.com/FieldmouseWorks/redshirt/issues/1) links
ongoing updates; implementation issues and PRs own their exact acceptance checks.

## Current surface

| Surface | Status | Evidence or next proof |
| --- | --- | --- |
| Conary fixture experiments | Implemented in Conary; draft under review | Real package sequences, independent checks, concrete replay and bounded reduction |
| External Redshirt runner | Implemented; PR #4 merged, attached-session refinement in draft #6 | Candidate-only selection, budgets, cancellation, evidence and concrete replay; Conary migration remains open |
| Browser gameplay adapter | Bounded private proof implemented | Ordinary movement, inspection and held transfers; independent checks and model-free replay |
| Comparative mechanics research | Versioned observations recorded | Readiness and hand-state comparisons; historical exceptions remain unresolved |
| Remote coordinator and additional providers | Future work | No implementation or deployment claimed |

## 2026-09-20 — attachment and replay capability

[Draft PR #6](https://github.com/FieldmouseWorks/redshirt/pull/6), implementation
head `ebd9ab9a81d718b6d9952dc7eec758310a484e45`, separates verified attachment to
an existing session from a resettable environment. Attached runs keep bounded
execution and mandatory checks but cannot emit a complete replay claim. Even a
forged complete trace is refused before setup/input. Existing resettable adapters
and v1 traces remain compatible.

`python3 -m unittest discover -s tests -v` passed 19 public synthetic tests. A
private integration passed eight fake-I/O checks and replayed two existing local
traces with zero selector requests. Its existing drag suite passed 107 checks,
including labelled mutants. A versioned reference accepted one held-input boundary
that the local baseline refuses; the contradiction, a successful ready control,
setup refusals and restored inventory are retained privately. Capture timing
within the gesture and historical equivalence remain unresolved. No gameplay
rule changed, and native observations have no executable-reset/replay claim.

The completed documentation/controller PRs #2/#4 are merged. Retired worktrees and
branches were removed after their evidence was preserved. No live model usage or
new allowance; no separate reviewer. Next: distinguish immediate capture from
capture on the first movement after readiness in one bounded private protocol.

## 2026-09-20 — external browser runner and transfer comparison

[Redshirt PR #4](https://github.com/FieldmouseWorks/redshirt/pull/4) is merged at
`56200ffffa3fb644f4b9a62fe04e1590f750abd1`, preserving implementation head
`1aea8e6379897a94da5ed5b11f750a91ba4ef571`. The standalone Python runner owns
candidate validation, budgets, cancellation, bounded evidence and concrete replay.
Adapters retain environment integration, setup, observations and independent
checks. Normal gameplay needs no runner, model, credentials or network provider.
Conary's Rust implementation is unchanged and has not been migrated.

At the implementation head, the local and hosted public synthetic suites passed
16 tests. The private browser proof passed 14 focused checks for movement,
inspection, stale/busy refusal, cancellation, hidden-state exclusion and a timing
negative control. A follow-up transfer slice passed 15 new checks plus a rerun of
the original 14. Four fixed transfer traces replayed with zero selector requests
and matching independent inventory/timing checks. Labelled lost-transfer and
shorter-timer controls were independently detected.

The follow-up application's existing local gates passed 1,009 default and 1,025
browser checks, with zero failures; the latter ran 12 suites. Its hosted default
lane passed 996 checks and skipped browser coverage. These counts belong to the
recorded runs, not a new execution caused by this documentation update.
All 35 follow-up evidence bundles passed manifests and per-episode bounds.

A separately versioned installed reference supplied three transfer observations
with overlapping readiness intervals. No speed advantage was detected for that
path. Its inventory presentation was restored; input/network/client delay,
unavailable server item IDs and lack of reset constrain the result. No native
executable-replay or historical-fidelity claim follows. All game/reference assets,
private source, client settings and captures stay outside this public repository.
See the [first proof update](https://github.com/FieldmouseWorks/redshirt/issues/1#issuecomment-5746185568)
and [transfer update](https://github.com/FieldmouseWorks/redshirt/issues/1#issuecomment-5746419667).

No live model was used in either browser slice. Earlier live-model allowances
remain closed; a future comparison needs a meaningful selection task, fixed
baseline and new concrete usage authorization. Next: a bounded unresolved input
or action-timing boundary. Cross-language Conary consolidation remains separate.

## 2026-09-20 — Conary exploration and model-free replay

The first working implementation extends Conary's existing `conary-test`
machinery. It supports scripted calibration, observation-driven seeded
exploration, optional Jev selection, recorded-operation replay and bounded trace
reduction. Controller-owned permissions, identity, state freshness, budgets and
mandatory final checks constrain every action. Independent domain checks decide
correctness.

Review: [Conary PR #1051](https://github.com/FieldmouseWorks/Conary/pull/1051),
head `4278a9202f2bc87f58d54547f5c03e37cf14d26f`,
base `2c4a21dda485c38ad05fd701934334a6b15d983f`.
The package binary was held at product source
`baa7b36ef690751f6a35d6b3ae8a763950ca8d01`; the harness used the PR head.

Three seeded runs and three Jev runs had the same eight-action ceilings,
fixture corpus, initial state and independent checks. The decision policy was
frozen before live use, with no replacement episodes.

| Measurement | Seeded trials | Jev trials |
| --- | --- | --- |
| Completed episodes | 3/3 | 3/3 |
| Distinct package states | 4 / 4 / 4 | 6 / 6 / 5 |
| State-changing operations | 4 / 3 / 5 | 7 / 7 / 7 |
| Median episode time, including reset/checks/cleanup | 19.123 s | 22.947 s |
| External model requests | 0 | 24 |

All 48 saved operations replayed without a model, using saved fixture bytes.
The six replays matched operations, independent checks, state coverage and
classifications. All twelve original/replay runs verified their baseline,
required final checks and cleanup. The disposable guest is powered off.

The live provider was `jev-1.13.0`: 24 HTTP requests, zero retries/fallbacks,
45,446 reported input tokens and 3,261 output tokens. The published-price estimate
was **USD0.001908732**; actual billing is unknown. The explicit provider validation
policy accepted 23 exact probability totals and one approximate total, preserving
the raw response. Approximation never grants execution authority.

### Verification and limits

- At the harness head, 370 library and 15 CLI tests passed; two existing tests
  remained ignored. Inventory, workspace Clippy, formatting, documentation,
  source-size gates and harness build passed.
- [Hosted CI](https://github.com/FieldmouseWorks/Conary/actions/runs/35472607089)
  completed with 39 successful jobs and two related failures. The pinned
  Tumbleweed prerequisite image was unavailable; its aggregate also failed.
  This remains a separate [Conary infrastructure issue](https://github.com/FieldmouseWorks/Conary/issues/971).
- The earlier real negative-control trace was reduced from nine operations to
  five and independently replayed at
  `9a58d5ce2948bf9b8f71ce7d32ee1124073e8525`. It was not rerun or reattributed to
  the current head. Earlier unsuccessful pilots retain their original results.
- This is a small positive result over a six-state corpus. It does not establish
  general model superiority, bug-finding ability, or an advantage over a stronger
  coverage-seeking algorithm. No new product defect is claimed.
- The independent checker detects a labelled negative control.
  [Conary #917](https://github.com/FieldmouseWorks/Conary/issues/917) was
  investigated but not reproduced by these fixtures. Bootable generation
  activation is outside the inert fixture corpus.

### Follow-up ownership

The browser proof above now supplies the second application. The standalone
Python runner owns its common controller/provider/evidence behavior; Conary still
uses its earlier Rust implementation. Consolidation must preserve Conary's
existing proofs and keep package semantics and game rules in their adapters.
A modern reference implementation supplies versioned observations, never sole
authority for historical behavior.

## Adding an update

For each meaningful capability change, record the date, issue/PR and exact
revision; the behavior demonstrated; commands actually run and measured results;
remaining failures or uncertainty; and one next useful action. Keep past failures
visible and distinguish proposed work from completed proof. Publish sanitized
summaries and deliberately shareable artifacts; credentials, private source
material and unrestricted run captures stay outside this public repository.
