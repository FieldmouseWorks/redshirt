# Redshirt project instructions

Read README.md, docs/ARCHITECTURE.md and the current entries in docs/PROGRESS.md,
then inspect the task's existing issues/PRs and actual branch/revision. Read
docs/RUST-ADAPTER.md, docs/INTERACTION.md or docs/JEV.md when their boundary changes.
Do not assume an open draft is merged or an old report is a new verification run.

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

## Evidence and verification

Public Redshirt is MIT. Keep private source/assets, credentials, installed client
files and unrestricted captures out of its code, tests, issues and PRs. Use
synthetic or explicitly sanitized evidence. Keep private counterpart details in
the consumer's own tracker; its private issue can link this public work.

Before changes, name the outcome/check and a bounded effort limit. Reuse existing
tests; record exact revisions, commands, failures and limitations. Run checks
appropriate to changed code as documented in docs/RUST-ADAPTER.md and docs/JEV.md;
documentation-only work needs link/diff review, not fresh live experiments.
Label self-review accurately. Preserve prior failed-target budgets and the active
session's delegation limits. Model choice remains optional and replaceable;
paid/live use requires its own concrete bounded allowance, never a past campaign's.
