# Redshirt progress

This page records demonstrated behavior, its limits and the next bounded slice.
The [progress thread](https://github.com/FieldmouseWorks/redshirt/issues/1) links
ongoing updates; implementation issues and PRs own their exact acceptance checks.

## Current surface

| Surface | Status | Evidence or next proof |
| --- | --- | --- |
| Conary fixture experiments | Implemented in Conary; draft under review | Real package sequences, independent checks, concrete replay and bounded reduction |
| Reusable Redshirt core | Extraction pending | Establish the smallest interface needed by a second real application |
| Browser gameplay adapter | Planned | One bounded scenario through ordinary controls, with player-visible observations and independently checked results |
| Comparative mechanics research | Planned | One unresolved question with versioned observations and explicit provenance |
| Remote coordinator and additional providers | Future work | No implementation or deployment claimed |

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

### Next application

Prepare one bounded browser-play and mechanics-research experiment. Reuse the
existing game's input and rule authority. Keep player-visible observations
separate from privileged evaluator state; record concrete actions and stop on
stale state, exhausted budgets or operator intervention.

Extract common controller/provider/evidence behavior into Redshirt only when
both applications demonstrate the need. Keep game rules and package semantics
in their adapters. A modern reference implementation can supply observations
and hypotheses with explicit provenance; it cannot by itself establish what a
historical version did. No game adapter or research result is implemented here yet.

## Adding an update

For each meaningful capability change, record the date, issue/PR and exact
revision; the behavior demonstrated; commands actually run and measured results;
remaining failures or uncertainty; and one next useful action. Keep past failures
visible and distinguish proposed work from completed proof. Publish sanitized
summaries and deliberately shareable artifacts; credentials, private source
material and unrestricted run captures stay outside this public repository.
