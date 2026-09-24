# Agent guide

## Start here

1. [README](README.md) and [current state](docs/CURRENT_STATE.md): what runs, what's open.
2. The assigned GitHub issue.
3. Only the docs for the boundary you're changing: [architecture](docs/ARCHITECTURE.md),
   [Rust adapter](docs/RUST-ADAPTER.md), [interaction](docs/INTERACTION.md), [Jev](docs/JEV.md),
   [native Jev](docs/JEV-RUST.md), [choice selectors](docs/CHOICE.md) or
   [comparisons](docs/COMPARISON.md).

## Invariants

- Rust owns the controller, budgets, cancellation, provider contract, evidence and concrete replay.
  One controller owns an episode; never nest controllers or copy one into an adapter.
- A provider selects one offered candidate. It never grants authority, supplies arbitrary
  commands or judges its own success.
- Default execution and replay work with no model and no credentials.
- Preserve cancellation, freshness, uncertain-attempt accounting, mandatory final checks/cleanup
  and evidence limits. Attach-mode sessions never claim reset or replay.
- Consumers (Conary, LoK-web, …) own their rules, content, roles, legal actions, environment
  setup and independent evaluators. Redshirt stays generic.
- Use typed values for decisions and test assertions, not substring matches over human-readable
  text. Negative tests assert the specific typed reason or run a positive control first.

## Consumer-driven work

A concrete need in a consumer project leads. Shared changes get their own Redshirt issue and
scoped PR with a generic contract and a synthetic reproducer; consumer-specific integration
stays in the consumer. Record nonblocking ideas as issues here without diverting the consumer's
task. See [consumer workflow](docs/ARCHITECTURE.md#consumer-driven-work).

## Workflow

- Short feature branch, scoped PR referencing the issue (`Refs #N`; `Closes #N` only when
  acceptance is fully met). The PR body is the record: what changed, checks run, limitations.
- Don't write evidence archives, hash receipts, task graphs or narration commits. Git history and
  the PR are the record.
- Keep [current state](docs/CURRENT_STATE.md) short and true; update it when behavior changes.
- The owner has authorized merging on green and cleaning up your own branches and worktrees.
- Stop and ask only for missing authority, unavailable capability or unresolved requirements.

Model choice and delegation are each contributor's machine-local setup; this file names none.

## Checks

CI ([test.yml](.github/workflows/test.yml)) runs three lanes on every push and PR; all must pass.
Locally, for the boundary you changed:

```sh
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked --all-targets -- -D warnings
cargo +1.98.0 test --locked
cargo +1.98.0 clippy --locked --all-features --all-targets -- -D warnings
cargo +1.98.0 test --locked --all-features
cargo +1.98.0 build --locked
REDSHIRT_BIN="$PWD/target/debug/redshirt" python3 -m unittest discover -s tests -p 'test_rust_*.py'
python3 -m unittest discover -s tests
```

Documentation-only changes need `git diff --check` and a look at changed links. Report skipped
tests; a green lane doesn't cover what it skipped. Consumer/browser integration is checked in the
consumer, not here.

## Boundaries

- Public MIT repository. No private source/assets, credentials, installed client files or
  unsanitized captures in code, tests, issues or PRs. Private counterpart details stay in the
  consumer's own tracker.
- No live provider/model calls without a fresh, bounded owner allowance. A past campaign's
  allowance never carries forward. Start with model-free and mock runs.
