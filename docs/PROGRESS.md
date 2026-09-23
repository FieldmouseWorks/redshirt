# Redshirt progress

This page records demonstrated behavior, its limits and the next bounded slice.
The [progress thread](https://github.com/FieldmouseWorks/redshirt/issues/1) links
ongoing updates; implementation issues and PRs own their exact acceptance checks.

## Current state

The Rust controller, external JSON sessions, bounded comparisons, optional native
provider, host-configured menus and diagnostic context comparisons are merged.
Python callers remain supported. The [surface inventory](#current-surface) and
dated entries link historical consumer evidence; merged runtime support does not
establish model usefulness or complete consumer migrations.

Recent completed repairs include
[completed-session process status #19](https://github.com/FieldmouseWorks/redshirt/issues/19)
and [terminal replay #31](https://github.com/FieldmouseWorks/redshirt/issues/31).
Their issue/PR records retain exact revisions and verification; they build on the
completed [integration batch #30](https://github.com/FieldmouseWorks/redshirt/issues/30).

Material limits: attached sessions cannot claim reset/replay; consumer rules and
independent checks stay in adapters; public CI has no browser gate.
[#26](https://github.com/FieldmouseWorks/redshirt/issues/26) remains the separate
live Choice contract disagreement. Prior live allowances remain closed.

The [standing workflow](WORKFLOW.md) was established in completed
[#28](https://github.com/FieldmouseWorks/redshirt/issues/28) and
[#29](https://github.com/FieldmouseWorks/redshirt/pull/29), including
[merge and cleanup authority](WORKFLOW.md#authority-and-effort).
The selected improvement is
[offline admission and check evidence #34](https://github.com/FieldmouseWorks/redshirt/issues/34).
Its canonical graph records the candidate, review, gates, merge and owned cleanup;
follow its next action while open. No subsequent outcome is assigned by this page.

## 2026-09-23 — offline admission and repeatable check evidence

[Issue #34](https://github.com/FieldmouseWorks/redshirt/issues/34) addresses concrete
gaps from the Jev workflow comparison. New context campaign admission checks that
the consumer's declared essential packet fits the selected-chunk and exact encoded
context-byte limits. An expected `insufficient` diagnosis explicitly marks an
abstention control. This structural check cannot discover omitted source
dependencies or establish semantic sufficiency; the consumer's
[corpus completeness audit](https://github.com/FieldmouseWorks/Conary/issues/1060)
remains separate. Historical saved runs retain their existing replay semantics.

Offline provider regressions preserve the sanitized
[#26](https://github.com/FieldmouseWorks/redshirt/issues/26) distribution, its
specific rejection, the invalid response receipt and the controller's refusal to
execute. They do not establish the server-side cause or resolve that issue.

The [workflow](WORKFLOW.md) adds a small command-receipt helper for actual local
working inputs, command results and raw output. It preserves failed checks and
detects input changes during a check; it does not infer test correctness or
implement agent dispatch, recovery or merge authority. The owning issue retains
negative controls, exact candidate checks, review and integration evidence.
No new live comparison or production confidence policy is part of this work.

## 2026-09-23 — concrete replay at a terminal observation

A consumer preflight found that a last checked operation could reach terminal
state and replay exactly, yet both controllers reported replay incomplete.
The terminal refusal preceded the exhausted-step check. The shared repair
recognizes consumed replay steps after observation/environment validation and
before terminal refusal; a terminal with remaining steps still stops normally.
Provider runs retain their actual terminal reason. Final verification, stable
identity, cleanup, per-step verdict equality and resource bounds remain required.

Generic counter regressions failed on the old ordering in both Rust and Python.
They now cover completed terminal replay, premature terminal refusal and failed
final verification. Focused Rust controller checks (11 tests) and Python checks
with HTTPX and real pipes (62 tests) pass locally. Full candidate and hosted
gate receipts, review and exact-main verification belong to
[issue #31](https://github.com/FieldmouseWorks/redshirt/issues/31) and its PR.
No live provider call, private consumer asset or game rule is part of this fix.

## 2026-09-23 — preserve process status after a completed session

[Issue #19](https://github.com/FieldmouseWorks/redshirt/issues/19) isolates a
completed-session shutdown race in the Python client. A valid terminal `done`
frame now leads to bounded natural process exit; unfinished sessions keep their
existing SIGINT cancellation and timeout escalation. The wire contract, Rust
controller and adapter cleanup ownership remain intact. See the
[client lifecycle](INTERACTION.md#async-client).

At baseline `1f544d440d17d6c5c78a7947dbdaf7e087d972db`, Python 3.14.4 completed
48 immediate-close and 24 settled-close synthetic Rust sessions without warnings.
On installed Python 3.12.14, four normal controls exited zero, while all 16
sessions with a 50 ms synchronous pause after `done` reproduced the exact warning:
Popen retained the true exit status zero, but asyncio reported 255. Episode final
checks and adapter cleanup passed in all cases, and no controller or adapter
process remained. No external reaper or live provider was introduced.

The installed 3.12 asyncio signal path calls Popen's polling signal method;
that poll can collect an exited child's status before the pidfd watcher. The
installed 3.14 path uses direct signaling and did not reproduce that route.
Waiting after `done` leaves status collection with asyncio. The original private
consumer's interpreter and exact schedule were not recorded, so this establishes
a matching shared-code failure without claiming its precise historical trigger.
The issue and PR retain regression negative controls, exact check results,
independent review and merge evidence. No general process-management rewrite or
warning suppression is part of this correction.

## 2026-09-22 — coverage-seeking baseline matches the completed Jev run

[Conary #1061](https://github.com/FieldmouseWorks/Conary/issues/1061) and
[PR #1062](https://github.com/FieldmouseWorks/Conary/pull/1062) add a 97-line
Rust `coverage-greedy-v1` selector using visible typed package effects and checked
attempt counts. It prefers untried transitions toward unseen package states;
projected outcomes never count as observations. Package policy remains
Conary-owned in its retained Rust pilot. No shared runtime or controller
migration was needed.

At Conary runtime `bc6824b9d2c9aba6debbe115573b583160408e38`, the completed
matched pair reached all six fixture states in eight-operation episodes:

| Policy | Actions to all six states | Full episode time | Provider calls |
| --- | ---: | ---: | ---: |
| Greedy Rust policy | 6 | 20.551 s | 0 |
| Jev 1.13.0, first episode | 7 | 23.000 s | 8 |

Both made seven checked state changes. Jev's second episode reached five states
in six operations, then its seventh response selected an option at 0.41 while
another had 0.42. The existing maximum-choice validator refused the proposal
before execution. This disagrees with the
[documented Choice contract](https://docs.typesafe.ai/api#choice-answer);
[issue #26](https://github.com/FieldmouseWorks/redshirt/issues/26) records the
provider follow-up. No validator relaxation, replacement choice or live retry
was made. The model response's server-side cause remains unknown.

The frozen campaign planned three repetitions per arm, eight actions each,
24 maximum HTTP requests, a USD0.07 conservative ceiling and a 30-minute approved
VM bound. It stopped on that provider failure, leaving three original episodes
unrun. **The full planned comparison is incomplete.** The same corpus, empty
baseline, pinned product, candidate generator, goal/history and independent
checker were used; no tuning followed live outcomes. A local orchestration-key
error before the first live request was corrected with the completed baseline
retained and a guarded continuation of unstarted episodes only.

All 22 completed operations and independent package checks matched model-free
replay. The failed episode's replay covers its concrete prefix, not its provider
failure. Six bundles, 54 artifact hashes and 23 checked decision contexts were
audited. Provider access was removed before replay; final checks, cleanup and
VM shutdown were verified. Actual usage: 15 requests, zero retries, 27,829 input
and 2,023 output tokens. Estimated cost was USD0.001168818 using the
[published price](https://docs.typesafe.ai/models) checked 2026-09-22; billed cost
is unknown and unused allowance is closed. No new package defect was found.

Recommendation: use the greedy policy for this fixture coverage workload.
The completed pair demonstrates no extra Jev value over it. One small corpus,
one completed pair and one failed live prefix do not establish general model
performance. The earlier gain over seeded selection remains historical evidence;
it does not establish superiority over a coverage-seeking algorithm.

Consumer self-review: 376 library and 16 CLI tests pass (two existing ignored),
plus inventory, workspace Clippy, fmt, doc truth, line caps and harness build.
The initial CLI-test compile error was fixed before live use. Conary's stacked
hosted gates await its parent; no new shared runtime test or live trial is
claimed by this documentation update. Implementation stayed in the parent
session with no delegated worker.

## 2026-09-22 — fresh lexical baseline and coding-model diagnostics

[Issue #24](https://github.com/FieldmouseWorks/redshirt/issues/24) and
[PR #25](https://github.com/FieldmouseWorks/redshirt/pull/25), runtime/tests
`9e9a5ed4eb15027a5510ea08bfc0b6b1e83faebe`, extend the
[context contract](CONTEXT-COMPARISON.md#version-2-lexical-retrieval-and-a-fixed-coding-model)
with deterministic Rust BM25 retrieval and fixed Codex diagnostics in both arms.
[Conary #1057](https://github.com/FieldmouseWorks/Conary/issues/1057) and
[PR #1059](https://github.com/FieldmouseWorks/Conary/pull/1059), corpus/exporter
`79ce07ed20718e5b971b258c3b45ebc242b1a89b`, own four fresh cases and independent
product checks. The first pilot stays frozen. At collection, these PRs were stacked on the
first pilot's unmerged PRs; no protected integration was bypassed.

Version 2 reserves Jev selector calls separately from CLI diagnostic turns.
The trusted CLI executable/catalog are hashed, execution tools disabled, and
responses restricted to one offered JSON choice. Rust bounds output, preserves
failure evidence, kills/reaps on cancellation and replays without either model.
Codex subscription billing remains unknown; its tokens/cache usage are separate
from Jev dollar estimates. CLI turns are not exact HTTP-attempt counts.

Injected protocol checks and local fake-server checks precede live collection.
The latter exercised the actual Rust CLI transport: one successful fake response
validated, and an injected HTTP 500 stopped after one request with retries set
to zero. No paid model call was used for those checks.

One separately bounded live campaign completed four Jev selections and eight
diagnostic CLI turns requesting `gpt-6-astra`, low effort, with no retry,
replacement, tool event or failure. The frozen manifest was
`0e260354d3eca545c109af6e5a624ad1b369e6e718710f7e63de5a7d19eb6626`.
BM25 resolved 2/4 cases and Jev-selected context 3/4: calibration 1/2 versus 2/2,
held-out 1/2 versus 1/2. Declared essential evidence retained was 6/8 versus 8/8;
insufficient-evidence answers were two versus one. Mandatory hashes matched.
Offline replay and a separate consumer audit verified all recorded requests,
choices, grades and accounting with zero model calls.

The improvement was one calibration case. Both arms abstained on a held-out
case whose declared essential set omitted a helper needed for the full answer;
8/8 labelled retention therefore did not establish complete evidence. The
consumer records this limitation and owns a
[fresh corpus completeness audit](https://github.com/FieldmouseWorks/Conary/issues/1060).
No post-run case replacement or regrading was performed. Keep Jev experimental;
this result does not support a production routing change or a held-out gain.

Jev used 30,739 input / 328 output tokens (USD0.001291038 estimated; full
reservation USD0.011010048 under USD0.012). Codex used 27,648 input / 97 output
tokens for baseline and 27,461 / 136 for treatment; both reported zero cached
input tokens. Subscription billing and combined dollar cost are unknown.
Median case time was 5.057 s versus 4.832 s, but total observed provider time
rose from 20.627 s to 22.290 s; four samples do not establish a speed advantage.
Campaign wall time was 43.871 s. Treatment used about 1% fewer actual context
bytes under equal limits. The allowance is closed; no further live calls are
authorized by this record.

Self-review passed 41 all-feature and 39 default Rust tests, both clippy modes,
fmt/build, the first pilot's live/mock replay and fresh mock replay. Eleven
context tests include lexical ranking, label isolation, mixed-provider replay,
strict transcripts, budget exhaustion, unexpected tools, output caps and process
cancellation. All six hosted checks passed at the runtime head. The consumer
passed 46 backing tests, exporter negative controls, router tests and doc truth.
No delegated coding worker was used; Codex only supplied benchmark diagnoses.
Raw host captures stay local. The CLI/profile pin does not guarantee a backend
snapshot or byte-identical ambient context; temporary session/path fields vary.

## 2026-09-22 — bounded diagnostic context comparison

[Issue #22](https://github.com/FieldmouseWorks/redshirt/issues/22) and
[PR #23](https://github.com/FieldmouseWorks/redshirt/pull/23), runtime/tests
`51ccd7b4b0ce059d2bf0bb5bcbf7bf0e3abe149c`, add the read-only Rust
[`redshirt-context` runner](CONTEXT-COMPARISON.md). Consumers own source-pinned
cases, actual owner packets, deterministic baseline order and independent truth.
Rust owns typed Jev judgments, immutable mandatory context, byte/chunk limits,
whole-campaign admission, per-call reservations, exact receipts and offline
replay. Ordinary action selection retains its existing contract. Native HTTPS
now honors validated host request-byte limits; a no-network regression covers
the previous hard-coded 16 KiB rejection.

The first consumer, [Conary #1055](https://github.com/FieldmouseWorks/Conary/issues/1055)
and [PR #1056](https://github.com/FieldmouseWorks/Conary/pull/1056), owns exporter
revision `9564efd458cdb8f039bb3209dd8162e71f5ba77b`, source revision
`180bd662516080b7523c9cee069396ae14b0f064`, and the full consumer report. Its
manifest was frozen as
`7b2e8c529c4b8138f739f6772c29f5b05f5e9ee895f2fc57313220bc548ed6e4` before collection.

One live campaign completed all 12 calls using `jev-1.13.0`, with no retry,
failure, fallback, replacement or tuning. Both arms used the same downstream
Choice, mandatory policy and context ceilings; treatment added one relevance
batch per case. Calibration accuracy was 1/2 baseline and 2/2 treatment;
held-out accuracy tied at 2/2. Labelled essential evidence retained was 3/8 versus
8/8; the baseline returned insufficient evidence once. Mandatory hashes matched
throughout, and offline replay reproduced all requests, selections, grades and
accounting with zero provider calls.

Baseline used 18,839 input tokens across four calls (USD0.000791238 estimated);
treatment used 47,095 across eight calls including selection (USD0.001977990).
Total estimated provider cost was USD0.002769228, with actual billing and cache
usage unknown. Full reservation was USD0.033030144 under the USD0.04 ceiling.
Median case provider time was 303.3 ms baseline and 572.8 ms treatment; total
campaign wall time was 4.675 s. The treatment retained about 6.5% more actual
context bytes under the same budget. All 32 typed answers validated with exact
probability totals. This live allowance is closed.

Self-review: `cargo test --locked --all-features` passed 35 tests, all-feature
clippy, fmt and build passed at the runtime head. Earlier slice checks passed
33 default Rust tests, default clippy, and 60 Python tests with one optional
HTTPX test skipped locally. The final code change strengthened only context
replay accounting and was followed by the full all-feature Rust suite. All six
hosted checks passed at runtime head. Consumer backing tests (8 diagnostics,
2 startup tests, 1 promotion test), exporter negative controls and documentation
checks passed. Five Rust context tests include label isolation, mandatory
context, malformed/cancelled calls, reservations, tampering and model-free replay.
No delegated worker was used.

This is a small positive evidence-selection result. The accuracy improvement was
on calibration; the two held-out cases tied. Four hand-selected closed-choice
cases, including two from one subsystem, cannot establish general coding-agent
quality or a production routing policy. Cost increased 2.5-fold, median latency
increased, and startup/cache effects are unmeasured. Next: a fresh consumer corpus
against a stronger deterministic baseline and a fixed downstream coding model,
under a new bounded allowance. Production context policy is unchanged.

## 2026-09-21 — host-configured wider action menus

[Issue #20](https://github.com/FieldmouseWorks/redshirt/issues/20), runtime/tests
`0716d9042d119816633396bfc685f1b4102947dc`, adds explicit host limits for candidate count, encoded candidate tables
and decision requests. Defaults remain 96/16 KiB/16 KiB; opt-in ceilings are
254/64 KiB/32 KiB. Optional provider requests can explicitly use up to 64 KiB;
16 KiB responses, call/time/input and total evidence bounds remain. Process
provider receipts reserve 256 KiB conservatively, so constrained evidence budgets
can stop earlier. No provider or client reply can raise host limits. Contracts
and compatibility requirements are in [the adapter boundary](RUST-ADAPTER.md#host-configured-menu-bounds).

Synthetic checks preserve complete 255-option Choice menus, exact byte/count
boundaries, pre-input candidate binding, both cross-runtime replay directions and
JSON envelopes larger than the previous client bound. Actual pipes use one Rust
controller; replay makes zero provider calls. No private consumer content is in
these fixtures. Self-review: default Rust 28/0, all-feature Rust 29/0; fmt and both
clippy modes pass. The complete Python suite passes 61/0 in 14.934s with the actual
Rust executable and optional HTTPX transport installed. Initial Python verification
was 60 passes plus one optional dependency skip; focused Python/controller checks
were 6/0 and 5/0. One local environment setup failed because ensurepip was absent;
uv populated a slice-local environment with pinned HTTPX 0.28.1, then the complete
suite ran without skips. No product check failed, and no assertion was weakened.

Commands actually run: `cargo build --locked --all-features`,
`cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo clippy --locked --all-features --all-targets -- -D warnings`,
`cargo test --locked`, `cargo test --locked --all-features`, and
`REDSHIRT_BIN=target/debug/redshirt python3 -m unittest discover -s tests -v`
(with absolute executable paths in the local run). Provider transports are injected;
no live call, delegated worker, deployment or model-quality claim. Exact review/CI
and merge receipts follow in the issue/PR. The blocked consumer owns its separate
integration proof and links the exact required shared revision privately.

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
| Optional Jev provider | Python PR #8 and native Rust PR #16 merged | Bounded typed questions, complete action menus and receipts; calibrated policy pending |
| Remote coordinator and further providers | Future work | No implementation or deployment claimed |
| Rust controller and adapter process | PR #10 merged; issue #9 completed | Existing browser adapter under one Rust owner, scripted/mock decisions and cross-runtime concrete replay |
| Rust external JSON sessions | PR #12 merged; issue #11 completed | Existing client/wire contract, role-filtered adapter views, cancellation and finite fractional replay digests |

## 2026-09-20 — native provider merged; bounded comparison runner

The owner authorized the next comparison slice. PR #16 merged at
`e5921928f12097afd037ee577c41bb19653573df` after self-review and all six hosted
checks passed at exact head `9e846a2fc79839791ff150d3fa06077d76d08724`.
Its earlier implementation and proof remain recorded below.

[Issue #17](https://github.com/FieldmouseWorks/redshirt/issues/17) adds a bounded
[Rust comparison runner](COMPARISON.md). Consumers supply fixed cases, visible-only
baselines and independent task labels. Rust checks matching initial requests
before model dispatch, reserves the whole campaign and each live case, retains
failure evidence, and reports task completion separately from invariant checks.
Confidence counts keep calibration and held-out cases separate and make no
counterfactual completion or calibrated-probability claim. The default uses local
manufactured responses and no key; live use remains explicitly bounded.

Self-review validation: 27 default Rust tests and 28 native-feature tests pass,
including five new comparison tests; both clippy modes and fmt pass. The existing
50 Python tests pass with one optional HTTPX test skipped locally. A private
four-case browser integration passes all eight baseline/mock episodes; its
independent goal, ordinary-input negative controls and zero-model replay remain
consumer-owned evidence. No delegated worker was used.

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
