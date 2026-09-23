# Shadow-ranking trial

This is the input-preparation and observation step in
[issue #42](https://github.com/FieldmouseWorks/redshirt/issues/42).

This trial compares a BM25 ordering with a separate Jev relevance ordering over
four frozen, source-backed coding questions. It sends every required source
excerpt and every candidate excerpt in each packet. Both rankings retain every
candidate; Jev scores the whole set. Nothing is selected, pruned, compacted, or
passed to a coding agent by this command. The exercise measures ranking
agreement and the added Jev-call latency and usage. It does not measure whether
an agent fixes code better, and it does not establish cache savings or a
cache-aware routing policy.

The four cases and source-reviewed evaluator key use source commit
`64d6348ddce3bfb10031155fe5de3957d09d38d2`. Each packet includes the complete
`AGENTS.md` file as required material and seven or eight candidate chunks. The
task prompts ask for regression controls where the key marks tests as core
anchors. An independent agent source review recorded in issue #42 evidence
accepted these declared core anchors; that was agent review, not human review.
The seven or eight candidates per task are curated source excerpts, and the
anchor labels are an evaluation convention rather than proof of unique semantic
necessity or that a model needed every marked excerpt. Inspect
`examples/shadow/essential-evidence-key.template.json` before scoring results.
The preparer binds a copy of that key to the actual manifest digest. It never
includes the key in the manifest or Jev request.

## Prepare and exercise locally

Use the implementation checkout for the tools and a separate detached worktree
at the packet source revision. The output directories below must not already
exist. Run these from the implementation checkout:

```sh
SOURCE=/tmp/redshirt-shadow-source
PREPARED=/tmp/redshirt-shadow-prepared
MOCK=/tmp/redshirt-shadow-mock
git worktree add --detach "$SOURCE" 64d6348ddce3bfb10031155fe5de3957d09d38d2
cargo +1.98.0 build --locked --bin redshirt-packet --bin redshirt-shadow
python3 scripts/prepare_shadow.py \
  --packet-binary "$PWD/target/debug/redshirt-packet" \
  --shadow-binary "$PWD/target/debug/redshirt-shadow" \
  --source-repo "$SOURCE" \
  --output "$PREPARED"
target/debug/redshirt-shadow \
  --manifest "$PREPARED/shadow-manifest.json" \
  --repo "$SOURCE" \
  --output "$MOCK"
target/debug/redshirt-shadow --replay "$MOCK"
```

Preparation builds each packet from committed source and runs shadow preflight.
Preflight checks all packet provenance, current referenced-file bytes, model
capacity, evidence bounds, and the four-call allowance before a live run. Build
summaries say `freshness: not_checked`; a packet is current for this trial only
after successful preflight or campaign verification against the pinned source
worktree. The mock campaign exercises the artifact path without contacting a
provider. Replay rechecks the saved manifest, preflight, calls, receipts, and
report offline, and makes no provider call. Replay explicitly does not recheck
current source freshness. Re-run preflight against the source worktree when
that claim needs refreshing. Capacity and evidence limits reject an oversized
request; they never shorten the supplied evidence to make it fit.

The prepared directory contains the full embedded manifest, packet files,
build summaries, preflight output, and the manifest-bound evaluator key. A
campaign output directory contains `manifest.json`, `preflight.json`, the raw
receipt rows in `calls.jsonl`, and `report.json`; keep these outputs outside the
repository. The report carries complete BM25 and Jev rankings, per-call
`elapsed_ms`, token usage, estimated input cost, error status, and aggregate
reservation/attempt counts. Per-call elapsed time covers Jev ask and response
validation after a source recheck. `wall_elapsed_ms` is collection time after
initial preflight and output setup, not full CLI duration. Capture outer command
duration separately if it matters. No standalone BM25 timing is recorded, so
this run cannot support a latency ratio or speedup claim.

## Live allowance

Review the mock artifacts and source-reviewed evaluator key before running
live; use a separate command and output directory:

```sh
cargo +1.98.0 build --locked --features jev-http --bin redshirt-shadow
: "${TYPESAFE_API_KEY:?Set TYPESAFE_API_KEY from your approved local secret store}"
LIVE=/tmp/redshirt-shadow-live
target/debug/redshirt-shadow \
  --manifest "$PREPARED/shadow-manifest.json" \
  --repo "$SOURCE" \
  --output "$LIVE" \
  --live
```

Live mode reads only the `TYPESAFE_API_KEY` environment variable for
credentials. Keep its value out of arguments, source files, logs, and reports.
The preparer removes that variable from child processes and does not need it.
There are four cases and at most one call per case; any failure stops the
remaining prefix and there are no retries. Preflight reserves at most
`$0.011010048` for the four maximum-token requests under the configured input
rate, below the manifest ceiling of `$0.02`. This is a conservative input
reservation, not a promise about a provider invoice; receipts record estimated
usage cost, while `billed_usd` remains unknown. The campaign does not reuse
cached judgments or measure cache hits.

Afterward, run `target/debug/redshirt-shadow --replay "$LIVE"` to validate the
saved evidence without another call. Compare every Jev rank with BM25 and the
separate source-reviewed key. Treat relevance ranking and coding correctness as
different outcomes; any later coding-agent evaluation needs its own held-out
tasks and independent correctness check. Keep the full packets and call
receipts available so the ranking analysis can be reproduced without asking
Jev again. When no further source recheck is needed,
`git worktree remove "$SOURCE"` removes the detached example worktree; Git
refuses the removal if someone changed it.
