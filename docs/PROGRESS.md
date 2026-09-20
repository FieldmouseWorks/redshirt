# Redshirt progress

This page records demonstrated behavior, its limits and the next bounded slice.
The [progress thread](https://github.com/FieldmouseWorks/redshirt/issues/1) links
ongoing updates; implementation issues and PRs own their exact acceptance checks.

## Current surface

The [consumer workflow](ARCHITECTURE.md#consumer-driven-work), tracked in
[issue#13](https://github.com/FieldmouseWorks/redshirt/issues/13), keeps shared
opportunities and required dependencies in Redshirt's own queue while a concrete
application outcome leads the work. This is a documentation/ownership update,
not a new runtime capability or another provider experiment.

| Surface | Status | Evidence or next proof |
| --- | --- | --- |
| Conary fixture experiments | Implemented in Conary; draft under review | Real package sequences, independent checks, concrete replay and bounded reduction |
| External Redshirt runner | Implemented; PRs #4 and #6 merged | Candidate-only selection, budgets, cancellation, evidence and concrete replay; Conary migration remains open |
| Browser gameplay adapter | Bounded private player API implemented | Broad visible choices, ordinary controls, independent checks and model-free replay |
| Comparative mechanics research | Versioned observations recorded | Readiness and hand-state comparisons; historical exceptions remain unresolved |
| Optional Jev provider | Python PR #8 merged; native Rust batches/abstention in draft #16 | Bounded typed questions, complete action menus and receipts; live usefulness/calibration pending |
| Remote coordinator and further providers | Future work | No implementation or deployment claimed |
| Rust controller and adapter process | PR #10 merged; issue #9 completed | Existing browser adapter under one Rust owner, scripted/mock decisions and cross-runtime concrete replay |
| Rust external JSON sessions | PR #12 merged; issue #11 completed | Existing client/wire contract, role-filtered adapter views, cancellation and finite fractional replay digests |

## 2026-09-20 — native Rust decision batches and confidence abstention

[Issue #15](https://github.com/FieldmouseWorks/redshirt/issues/15), implementation
`088bf92621e4d76894547e796397e948a0f11b58`, adds shared Rust question/answer types,
batch validation, caller-configured confidence policy and native optional Jev
HTTPS. It builds on the now-merged Rust controller and consumer workflow;
[draft PR #16](https://github.com/FieldmouseWorks/redshirt/pull/16) remains separate
from that integration. See [the provider contract](JEV-RUST.md).

One request carries the full current action Choice and up to seven independent
Choice/Score/Noul questions against the same authorized observation. Additional
answers remain advisory evidence. Below an explicitly configured confidence floor,
the provider records its proposal and returns `provider_uncertain` before input;
there is no default cutoff, candidate pruning or automatic fallback. Rust retains
call/evidence reservations and interrupted receipts, including mandatory final
checks and cleanup. Existing Provider, adapter and replay contracts are unchanged.

The final default Rust suite passes 22 tests; the native-feature suite passes 23.
Fmt and default/native clippy pass. The existing 50-test Python run passes with
one optional HTTPX transport test skipped locally. The Rust synthetic consumer
produces the same one checked operation under scripted and three-question batched
selection: two provider calls, one input, then concrete replay with zero provider
calls. Low confidence, stale/busy state, missing effects and cancellation retain
the expected refusals and finalization. Invalid auxiliary answers refuse the entire
batch. Native HTTPS construction and sensitive request headers are tested without
network; old dependency versions remain locked alongside optional new packages.

The first size-limit test fixture did not actually exceed the request bound; its
failure is retained, the fixture was corrected, and focused/full suites pass.
Self-review also bounded diagnostic expansion caused by repeated credential
redaction or invalid UTF-8, with explicit evidence flags and a regression check.
Implementation and review were kept in the parent session after owner steering;
no delegated findings were adopted. No live Jev calls, threshold calibration,
consumer migration, merge or deployment occurred. These results establish the
shared Rust mechanism; model usefulness still requires its own bounded comparison.

## 2026-09-20 — reviewed runtime stack merged

The owner authorized integration and cleanup. PRs #6, #8, #10 and #12 are merged
in dependency order; runtime main is `60978f79165b5a693ea6cd2876a628b90979b0c8`,
with the same tree as the tested implementation
`2526c5745318e727bafe2e9895d516c4c275e89a`. Each merge checked the destination,
expected head, hosted checks and resulting tree; merge commits retain ancestry.
Historical entries below describe their original revisions and draft status.

Fresh local verification of that implementation passed nine Rust tests,
`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and
50 Python tests with `REDSHIRT_BIN` set (no skips). The first Python invocation
omitted that variable and skipped 14 process tests; the complete rerun passed.
Fresh consumer checks passed 30 gameplay and 46 JSON integration assertions,
including role filtering, negative controls, concrete replay with zero provider
calls, and successful process exit. Raw consumer evidence remains private.

This was self-review, with no delegated worker or live provider use. Existing
failed-target budgets and implementation limits remain in force. Documentation
PR #14 records the consumer workflow and this integration result. Optional Rust
decision batching in [issue #15](https://github.com/FieldmouseWorks/redshirt/issues/15)
is separate, unmerged work; current consumers do not depend on it. The next
consumer improvement can use the merged boundary without waiting for another
provider experiment. No release, deployment or model-quality claim is made.

## 2026-09-20 — external JSON sessions under Rust

[Issue #11](https://github.com/FieldmouseWorks/redshirt/issues/11) moves external
decision sessions onto the same Rust owner through `--stdio`. The existing
Python client, observation/tool schema and decision/done envelopes are unchanged.
Unix asynchronous pipes allow cancellation and timeout with stdin still open;
partial frames and interrupted decision receipts survive cancellation. There is
no second controller, network listener or added environment authority.

Nine Rust tests and50 Python tests pass, including exact wire comparison against
the Python baseline, malformed/stale/prequeued replies, EOF, SIGINT, deadlines,
failed effects and concrete replay. Fmt/clippy pass. An independent Python JSON
oracle checks finite samples from16,384 seeded binary64 patterns and numeric edge
cases. Finite fractional views now preserve Python-compatible v1 hashes without
rounding; the prior integer-only restriction is superseded. Integer traces remain
compatible. Arbitrary precision and normalized real-time replay are not claimed.

The existing private browser scenario passes30 gameplay/role checks and46 focused
cutover checks. Player wire projections match the explicit Python baseline
apart from random tokens; five ordinary actions still cost seven input units and
six choices. All available role launchers run under Rust. Fractional diagnostics
retain their values and grants. Omitted movement fails independent checking;
stale/changed-role choices and role injection refuse before input. Hidden-state
changes leave the complete authorized session projection identical. Cross-runtime
player replay has zero provider calls;111 episode artifact hashes match.

One public fixture repair corrected the stale mutation point; its failed log is
retained privately. The first browser verifier emitted child-watcher warnings
on redundant close-time signalling after process exit. Dedicated sessions await
and record normal exit, including every available role. No client change or game
rule change was required. Self-review only; no delegated reviewer, live model
call or native-client observation. Optional captures and other callers remain on
their explicit baselines; Conary is unchanged. Next: review the stacked drafts
and use this same boundary for the pending bounded provider comparison when new
live usage is authorized.

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
