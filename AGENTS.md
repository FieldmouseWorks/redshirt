# Redshirt project instructions

Start in the actual Git checkout (`repo/` or an owned task worktree), not its
parent directory. Inspect applicable `AGENTS.override.md`/`AGENTS.md`, working
state, exact revision and the task's existing issue/PR before changing files.
Use [current state](docs/PROGRESS.md#current-state) for continuation and the
[workflow](docs/WORKFLOW.md) for substantive dependent work. Read only what the
task needs: [architecture](docs/ARCHITECTURE.md) for ownership/cutovers,
[Rust adapter](docs/RUST-ADAPTER.md), [interaction](docs/INTERACTION.md),
[Jev](docs/JEV.md), [native Jev](docs/JEV-RUST.md) or
[comparisons](docs/COMPARISON.md) when that boundary changes. An open draft is
not merged behavior; historical receipts are not fresh verification.

## Goals and ownership

Build reusable, bounded experiment and interaction tooling from concrete consumer
needs. Rust owns the durable controller/provider/budget/evidence/replay core;
versioned interfaces let useful Python, browser and other integrations remain
independent. Preserve working baselines until a bounded cutover proves parity.

Consumer projects own their rules, content, role/actor policy, legal actions,
environment setup and independent correctness checks. One controller owns an
episode; a provider selects an offered candidate and never grants authority or
judges its own success. Default runs and concrete replay work without a model.
Preserve cancellation, freshness, uncertain-attempt accounting, mandatory final
checks/cleanup and evidence limits. Attachment is not reset/replay authority.

## Work arising in another project

Follow [the consumer workflow](docs/ARCHITECTURE.md#consumer-driven-work).
One concrete application outcome may lead a slice. Search the existing Redshirt
issues before adding a shared need; record demonstrated limitations separately
from hypotheses, with a generic contract and acceptance check. Required shared
changes get their own issue, branch, commit, checks and scoped PR here. Record
nonblocking opportunities here without diverting the consumer's active task.
Do not copy a controller into an adapter or make a speculative platform/context
graph a prerequisite for application work. Other repositories keep their scope.

## Roles and completion

Use `gpt-6-astra` / `max` for primary conversation, planning, architecture, graph
selection, review and integration; `gpt-6-sol` / `max` for complex implementation
and debugging; `gpt-6-luna` / `max` for bounded exploration, documentation and
checks. Delegate when useful, with exact inputs, acceptance, file ownership and
concurrency limits; workers must preserve others' changes. Record requested and
observable actual routing, report unavailable routes without substitution, and
verify artifacts rather than accepting completion claims. Instructions cannot
change an already running model. DeepSeek remains paused until explicitly enabled.

Define the outcome, acceptance and bounded effort before editing. Keep one
canonical graph in the owning issue body for substantive dependent work; simple
changes need only a short plan. Follow the [graph and evidence procedure](docs/WORKFLOW.md#task-graph).
Verified results unlock dependencies; failed checks get scoped repairs within
the existing limit. Archive completed graphs, keep one next action, and continue
through ready authorized work. This guidance does not implement a dispatcher.

Complete the diff/behavior review and required gates, then the authorized PR,
merge, exact-main verification, evidence read-back and owned cleanup. Preserve
unrelated branches, worktrees and processes. Reuse recorded authority; a graph
edit cannot grant permissions or reset effort. [Authority and effort](docs/WORKFLOW.md#authority-and-effort)
records this setup's scope and how to handle a genuinely missing decision.

## Evidence and checks

Public Redshirt is MIT. Keep private source/assets, credentials, installed client
files and unrestricted captures out of its code, tests, issues and PRs. Use
synthetic or explicitly sanitized evidence. Keep private counterpart details in
the consumer's own tracker; its private issue can link this public work.

Use original sources for source claims and actual revisions, command outputs and
receipts for implementation claims. Preserve failures and distinguish self-review,
independent review, local, browser and hosted results. Relevant input changes
invalidate affected acceptance; never relabel old evidence as current.

- Documentation only: `git diff --check`, changed links/anchors and actual diff
  review; no fresh runtime or live experiment is required.
- Python: `python3 -m unittest discover -s tests -v`.
- Rust: `cargo +1.98.0 fmt --all -- --check`, default and all-feature
  `cargo +1.98.0 clippy --locked --all-targets -- -D warnings` and
  `cargo +1.98.0 test --locked`; use the exact feature variants and real-pipe
  commands in the [check matrix](docs/WORKFLOW.md#checks).
- PR publication retains all three [CI lanes](.github/workflows/test.yml),
  including optional HTTPX and the built Rust/Python pipe checks. Consumer/browser
  integration has its own conditional gates; CI does not establish those passes.

Preserve failed-target budgets and host permission controls. The optional product
provider remains replaceable; paid/live use needs its own concrete bounded
allowance, never a past campaign's. Keep private counterpart evidence in consumers.
