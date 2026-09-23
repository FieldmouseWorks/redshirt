# Evidence-driven work

This is a working procedure for bounded Redshirt changes. It uses the existing
GitHub issue, PR and progress records; it does not install a scheduler, automatic
dispatcher, checkpoint service or unattended restart. The current setup graph is
[issue #28](https://github.com/FieldmouseWorks/redshirt/issues/28). The public
[progress thread](https://github.com/FieldmouseWorks/redshirt/issues/1) is an
index, not a second task graph.

Keep the architecture intact while following this procedure: Rust owns the
durable controller, budgets, cancellation, provider contract, evidence and
concrete replay. A single controller owns each episode. A provider may select
only an offered candidate; it never grants authority or judges success. Consumers
own their rules, content, roles, legal actions, environment setup and independent
evaluators. Default execution and replay remain model-free. See the
[architecture](ARCHITECTURE.md) and its [consumer workflow](ARCHITECTURE.md#consumer-driven-work).

## Start with the outcome and its owner

1. Start a fresh Codex session in the actual checkout and inspect the current
   branch, exact revision and dirty state. Apply machine-wide
   `AGENTS.override.md` when present, or fall back to machine-wide `AGENTS.md`.
   From repository root through the current directory, apply that directory's
   `AGENTS.override.md` when present; otherwise apply its `AGENTS.md`. Choose one
   per directory rather than combining both. A pointer identifies an ordinary
   Markdown document to open; the pointer alone does not load it. Read documents
   as task context, not as instruction files unless explicitly designated. See
   the [AGENTS.md guide](https://learn.chatgpt.com/docs/agent-configuration/agents-md).
2. State one concrete outcome, its owner, and observable acceptance checks before
   implementation. Application rules and independent checks stay with the
   consumer. Search existing Redshirt issues first; update the existing owner or
   open one bounded follow-up for a demonstrated shared limitation. Label a
   hypothesis as such, keep a synthetic or sanitized reproducer, and say whether
   the active consumer outcome depends on it.

## Task graph

For a substantive outcome with dependent steps, keep its one canonical working
graph in the owning issue body. Put a stable ID and outcome, dependencies, owner
and requested model/effort, input references, acceptance check, state, evidence
references, and either a bounded effort limit or an explicit owner-authorized
effort waiver on every node. An explicit owner waiver may remove the numeric
effort limit for its named outcome. A waiver is not inherited by later work. Use
states such as `queued`, `ready`, `running`, `review`,
`blocked` and `done` consistently. The PR, progress index and handoff link to the
graph and summarize status; they do not copy it. A simple independent task needs
only a short plan and acceptance check in its issue or PR.

Astra at max reasoning selects ready work and owns architecture, review and
integration. Sol at max reasoning handles complex implementation; Luna at max
reasoning handles bounded documentation or test work. State the requested model
and effort for each assignment, and record the actual model/backend only when
the runtime exposes it. A requested native route being accepted is not backend
attestation. If a route is unavailable, report that fact without substituting
another model. Text in a file or prompt cannot switch an active model. DeepSeek
stays paused unless the owner explicitly re-enables it. See
[Rethinking skills and prompts for GPT-6 Astra](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra).

## Select, execute and review work

Astra selects a ready node only after its dependencies and inputs are available.
Parallel work is appropriate only when inputs and file ownership do not conflict.
The owner of a node checks the observed result against its acceptance evidence;
an agent's completion statement alone does not unlock dependent work. Inspect
every child in a completed batch before marking the batch done.

Use existing feedback first. Add a new check only to resolve a concrete
observation, and include a meaningful negative control when the claim needs one.
If a dependency is missing, use an isolated, authorized setup. Python 3 is
available in this environment; `python` may not be. An environment-local `uv`
install is the recorded fallback when `venv` lacks `ensurepip` (see the current
[progress entry](PROGRESS.md)). Keep concurrent Rust builds in
separate `CARGO_TARGET_DIR` directories and use the matching absolute executable
path for `REDSHIRT_BIN`.

Before expensive gates, freeze the exact candidate revision and review its diff.
Then run local and hosted checks concurrently only when builds, databases, ports
and output paths are isolated. Preserve required hosted gates; don't add CI or a
test framework just to make this procedure look more official. An edit that can
affect an acceptance result makes that result stale: rerun the affected gate or
label the old receipt as belonging to the earlier revision.

For each completed or failed node, record the exact source reference behind
source claims and the exact revision behind implementation claims. A useful
receipt includes commit/tree identity, command, working directory, relevant
environment and dependency versions, start/end time or duration, exit status,
result summary, hashes for relevant artifacts, and a link to raw output or
artifact. Inspect raw results before recording a conclusion. Preserve failures
and limitations. Label self-review as self-review; distinguish it from an
independent review. Make performance or speedup claims only from comparable,
completed measurements, and state the measured scope. Keep private evidence in the
consumer's private tracker or storage, with only a sanitized public summary and a
link allowed by its owner. Never put private source/assets, credentials, installed
client files or unrestricted captures in Redshirt code, tests, issues or PRs.

When a check fails, create a scoped repair against that acceptance check and use
the node's remaining bounded effort. Do not quietly broaden the task, lower the
check, reset a budget or claim a pass from an earlier revision. When blocked,
record the exact missing input or failed condition and the next action that can
resolve it. When a new opportunity is not required by the selected outcome,
record it in the appropriate issue/queue and keep working on the selected outcome.

## Checks

Run local gates relevant to the changed boundary, and preserve all required
hosted gates for every PR, including documentation-only PRs. The existing
[test workflow](../.github/workflows/test.yml) runs three jobs on every push and
pull request: Rust, synthetic Python, and optional Jev transport. Its pinned Rust
toolchain is 1.98.0. The equivalent Rust commands are:

```sh
rustup toolchain install 1.98.0 --profile minimal --component clippy,rustfmt
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked --all-targets -- -D warnings
cargo +1.98.0 test --locked
cargo +1.98.0 clippy --locked --all-features --all-targets -- -D warnings
cargo +1.98.0 test --locked --all-features
cargo +1.98.0 build --locked
REDSHIRT_BIN="$PWD/target/debug/redshirt" python3 -m unittest discover -s tests -p 'test_rust_*.py' -v
```

The workflow's Python jobs run:

```sh
python3 -m unittest discover -s tests -v
python3 -m venv .venv
.venv/bin/pip install '.[jev]'
.venv/bin/python -m unittest discover -s tests -v
```

The ordinary Python lane can skip HTTPX transport and Rust-pipe tests when their
optional dependency or executable is absent. The optional transport job installs
`.[jev]` and tests the injected transport without a live request; it does not set
`REDSHIRT_BIN`, so Rust-pipe tests still skip there. The Rust lane's final command
sets the executable and exercises actual adapter pipes and Python/Rust replay.
For a custom or isolated target directory, substitute its actual absolute binary
path. Report skips; a green lane does not establish coverage for tests it skipped.

Documentation-only changes need `git diff --check`, manual review of the actual
diff, and inspection of relative links and anchors; there is no existing
automated documentation-link checker. They do not need a fresh runtime or live
experiment. Keep local default and all-feature Rust results distinct, and report
browser, hosted and consumer checks as separate gates; one does not stand in for
another. Browser and other consumer gates apply when a boundary change requires
them. Private browser evidence does not become a new public check by being
mentioned here. A live provider call requires its own concrete authorization and
allowance; a previous campaign's allowance does not carry forward. Keep default
execution and replay model-free.

## Authority and effort

The setup scope comes from the owner's request recorded in [issue #28](https://github.com/FieldmouseWorks/redshirt/issues/28),
under the existing `AGENTS.md` rules for bounded local work, private-evidence
handling and applicable checks. That authorizes this documentation, its local
checks and scoped issue/PR preparation. On 2026-09-23, the owner added standing
permission for Redshirt: "You have standing permission to merge and clean up on
green." After review and all required checks pass for the exact candidate in an
authorized outcome, continue through merge, exact-main verification, evidence
archive/read-back and cleanup of owned merged resources without another approval
pause. [Issue #28](https://github.com/FieldmouseWorks/redshirt/issues/28) records
the decision and setup receipts. This permission does not select unrelated queue
items or grant deployment, release, newly paid service or live-use allowance.
Any future live use needs its own concrete, bounded
authorization. Unrestricted filesystem access is a capability, not additional
project authority.

This setup's effort limit is one document pass and at most two scoped repairs per
failed acceptance gate, counted across child assignments. Future work records its
own bounded limit or explicit owner waiver in the canonical graph; no limit or
waiver carries over from this setup. Record a waiver as an actual owner decision,
not an inference from silence or an old allowance. An effort limit or waiver does
not alter scope, acceptance checks, permissions or failed-target evidence.

## Resume and close the outcome

On resume, reopen the canonical issue, inspect the exact branch/revision and
uncommitted diff, reconcile every node with its evidence, and check whether any
process is still running before continuing. Do not reuse stale or failed receipts
as current passes. Complete work only after the acceptance evidence and required
review exist at the candidate being proposed. Follow the authorized PR, hosted
checks and owner decision. When merge is authorized, verify the exact remote-main
revision and its required checks. Archive the final evidence, read it back, and
verify the recorded artifact hashes. Then remove only this task's merged branches
and clean worktrees, after confirming their commits are ancestors of remote main
and checking dirty state. Stop or remove only processes owned by this task, after
inspecting them. Preserve unrelated work.

Before changing an issue body to a new active outcome, preserve the completed
graph and its final receipts in an issue comment. Keep one clear next action in
the issue and update the progress index to point at the owner and proof. Archive
the setup graph and receipts before starting a new outcome. Finish this setup
after its document acceptance review and the existing hosted checks on its PR.
Do not select unrelated existing issues as continuation.

When an observed result justifies a workflow improvement, record the problem, the
smallest useful change, and its review/check in the owning issue. Treat these
instructions and graphs as human-guided coordination: no automatic enforcement,
model dispatch, checkpointing or restart exists unless a corresponding executor
is separately implemented and verified.
