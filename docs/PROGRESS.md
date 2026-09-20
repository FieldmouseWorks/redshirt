# Redshirt progress

This page records demonstrated behavior, its limits and the next bounded slice.
The [progress thread](https://github.com/FieldmouseWorks/redshirt/issues/1) links
ongoing updates; implementation issues and PRs own their exact acceptance checks.

## Current surface

| Surface | Status | Evidence or next proof |
| --- | --- | --- |
| Conary fixture experiments | Implemented in Conary; draft under review | Real package sequences, independent checks, concrete replay and bounded reduction |
| External Redshirt runner | Implemented; PR #4 merged, attached-session refinement in draft #6 | Candidate-only selection, budgets, cancellation, evidence and concrete replay; Conary migration remains open |
| Browser gameplay adapter | Bounded private player API implemented | Broad visible choices, ordinary controls, independent checks and model-free replay |
| Comparative mechanics research | Versioned observations recorded | Readiness and hand-state comparisons; historical exceptions remain unresolved |
| Optional Jev provider | Draft #8, mock/default verified | Bounded Choice transport, raw receipts and usage; new live comparison pending |
| Remote coordinator and further providers | Future work | No implementation or deployment claimed |
| Rust controller and adapter process | First bounded parity proof implemented; issue #9 | Existing browser adapter under one Rust owner, scripted/mock decisions and cross-runtime concrete replay |

## 2026-09-20 — Rust controller ownership

[Issue #9](https://github.com/FieldmouseWorks/redshirt/issues/9) adds the Rust
library/executable and [trusted adapter method boundary](RUST-ADAPTER.md). Rust
owns budgets, freshness, uncertain-attempt accounting, final checks, evidence and
replay. Python hosts adapter methods and the optional existing provider transport;
it never runs a second controller in this path. Conary's demonstrated admission
and finalization invariants retain their MIT provenance, without domain types.

Seven Rust tests and43 Python tests passed, including six real-pipe cases.
Checks cover Stop, stale/busy refusal, cancellation before and during input,
cancelled-provider receipts, uncertain attempts, attached-session restrictions,
negative effects, artifact hashes and replay in both directions. Self-review
added failing controls for a changed reset capability on Stop and a v1 control-
character encoding mismatch; both pass after one repair cycle. Clippy and fmt
passed. No separate reviewer or delegated worker was used.

The existing private browser scenario passed18 focused checks under the corrected
binary. Scripted Rust and injected Jev mock selection match the Python baseline's
five ordinary actions, seven input units, six choices and independent outcomes.
Rust replay and Python/Rust cross-replays consult no provider. Omitted movement
is independently detected; stale/changed-role choices refuse; hidden-state
changes leave the outbound request identical. No captures or live model calls.

This proves one controller boundary, not a completed migration of every caller.
The first Rust path requires zero captures and integer-valued observation JSON
for portable v1 replay hashes. Existing interactive callers and Conary remain on
their respective baselines. Game authority, private source/assets and installed
clients remain outside Redshirt. Model usefulness and historical fidelity are
separate questions. Next: move the existing selector-facing JSON session onto
this proven Rust owner while preserving its client contract.

## 2026-09-20 — local JSON interaction and capability-filtered clients

[Draft PR #8](https://github.com/FieldmouseWorks/redshirt/pull/8), implementation
head `235fdc0889e9d58c5f7aabdab3ef7ede7c3b1246`, adds a persistent local JSON-lines
interface and async client. External tools receive the adapter's authorized
observation and a current action schema, then return one ID. The existing runner
retains budgets, freshness, independent checks, evidence and replay. No parallel
controller or network listener was added. See [the protocol](INTERACTION.md).

All37 public tests passed, including real subprocess pipes, stale/prequeued and
malformed replies, disconnect/cancellation, failed effects and zero-provider
replay. The first run's two subprocess import-path errors were corrected in the
test fixture. Both [hosted lanes](https://github.com/FieldmouseWorks/redshirt/actions/runs/35486316132)
passed at the implementation head.

The private integration passed44 API checks,30 focused checks and its1,026-check
browser gate. Five player traces replayed without a model while screenshot and
observation-time pixel reads were forbidden. Scoped observer, player, support,
tester and GM sessions exposed their permitted tools; unsupported actor control
refused access. Domain roles and data projections stay in that private adapter.
Normal UI pixels matched the pinned baseline; missing input, forbidden roles and
an unrelated repaint were negative controls. A missing-hover checker type error
was retained and corrected before the final pass.

No live model, separate worker/reviewer, deployment or merge occurred. These are
local capability gates, not remote account authentication. The owner then accepted
a [Rust shared core with flexible integrations](ARCHITECTURE.md). The Python proof
and its model-neutral wire contract remain the migration baseline. Next is a
bounded Rust parity proof using the existing adapter, before retiring any controller
or expanding shared machinery. No Rust migration is claimed by this draft.

## 2026-09-20 — bounded Jev provider and ordinary gameplay choices

[Draft PR #8](https://github.com/FieldmouseWorks/redshirt/pull/8), implementation
head `85308ea14954616a1a7fe48a93b14a8ceb8b1206`, adds the optional pinned Jev
Choice provider. The controller retains bounded provider receipts through failure
or cancellation and reserves their evidence space before dispatch. Up to 96
adapter candidates plus stop support ordinary interactive screens; existing byte,
input, time and replay restrictions remain. The provider has an explicit call cap,
a five-second absolute deadline, one in-flight request and no retry or fallback.
Conary's approximate-total policy and MIT notice are retained; its Rust pilot is
unchanged. See [provider contract](JEV.md).

All 29 synthetic tests passed with the optional HTTPX dependency. The ordinary
stdlib lane needs no provider package and skips only the HTTPX transport test.
The optional transport is tested in memory: no live request was made. A local
wheel build includes both license notices; no package was published.
[Hosted checks](https://github.com/FieldmouseWorks/redshirt/actions/runs/35484040411)
passed both lanes at the implementation head.

A private integration now exposes a model-neutral player API with 32 choices on
its initial screen, audited visible state and ordinary controls. It passed 29 API
checks, ten adapter checks, and three concrete replays without model calls. Its
full browser gate passed 1,026 checks across 13 suites; normal pixels match the
captured baseline. Hidden-state exclusion, stale/intervention refusal and a
missing-input negative control were exercised. Game rules, private source/assets
and captures remain outside this public repository.

These are implementation and mock-transport results, not Jev gameplay or a model
advantage claim. A new bounded local comparison and secure credential provisioning
remain pending. Native broader play is still follow-up and has no reset/replay
claim. No workers, separate reviewer, deployment or merge were used for this slice.
Next: the approved/provisioned local gameplay comparison through this interface,
then broader native observation/action coverage without copying game logic here.

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
