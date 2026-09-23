# Source-backed evidence packets

`redshirt-packet` saves caller-selected source material as an explicit, reusable
packet. Each reference records a repository-relative path and inclusive line
range. The built packet retains the exact excerpt, a hash of the whole source
file, and a hash of the excerpt. `verify` checks the packet against its recorded
Git revision and the referenced files in the supplied working tree.

Packets make their supplied material recoverable and freshness-checkable. The
caller chooses the task, required instructions, paths, ranges, and required vs.
supporting groups. The tool does not discover dependencies, decide whether the
packet is semantically complete, rank or trim evidence, summarize it, or call a
model. A packet grants no tool or action authority. A successful check establishes
freshness only at the time of verification; it does not lock files against edits
during later use.

## Build and verify

Build reads the exact committed blobs named by `source_revision` and writes a
new packet. Its summary reports `freshness: "not_checked"`: creating a historical
packet does not claim that the current working tree matches it. Verify requires
the repository `HEAD` to equal the packet revision and the referenced working
files and excerpts to match the saved hashes. It checks `HEAD` before and after
reading the referenced files one by one. Those reads are sequential, so verify
is not an atomic snapshot across files. It reports `freshness: "current"` only
for that check. Reverify as close as practical to using a packet, since later
edits are not locked out.

The spec uses version 1, a 40-character lowercase commit ID, a task, and two
caller-authored lists. Each reference has a stable `id`, repository-relative
`path`, and inclusive `start_line` and `end_line`. The checked-in
[`terminal-replay.json`](../examples/packets/terminal-replay.json) is a complete
example of that format.

Here the full instruction file is explicitly required, and the complete controller,
contract, and controller test files are included to avoid selecting isolated lines
from the replay path. These line counts are from commit
`5b07a87d9ebeda7fead8d3384cfc22417d10c17b`. The task and source list are an example
of caller judgment, not an audit that every dependency is present. The packet's
original excerpts remain intact in both groups.

The checked-in example is a spec, not a copied source bundle. Build its packet
into a fresh external output path. Build the binary from a checkout that contains
`redshirt-packet`; `--repo` points at the source tree whose revision and files are
being checked:

```sh
cargo +1.98.0 build --locked --bin redshirt-packet
git worktree add --detach /tmp/redshirt-packet-5b07a87 \
  5b07a87d9ebeda7fead8d3384cfc22417d10c17b
target/debug/redshirt-packet build --repo /tmp/redshirt-packet-5b07a87 \
  --spec examples/packets/terminal-replay.json \
  --output /tmp/terminal-replay-packet.json
target/debug/redshirt-packet verify --repo /tmp/redshirt-packet-5b07a87 \
  --packet /tmp/terminal-replay-packet.json
```

Only remove that exact worktree if you created it, it is listed by `git worktree
list`, and `git -C /tmp/redshirt-packet-5b07a87 status --short` is empty. Use the
ordinary `git worktree remove /tmp/redshirt-packet-5b07a87` command; do not force
removal or clean another worktree.

The JSON summary includes `packet_sha256`, the canonical encoded packet JSON
digest (not the pretty-printed file-byte hash), freshness status,
required-reference count, chunk-reference count, and distinct source-file count.
Build and verify make no provider calls. Keep task packets and command summaries
outside the checkout; the checked-in example spec contains paths and line ranges
only.

## Limits and boundaries

The version-1 artifact has finite I/O limits. Inputs that exceed a limit are
rejected; the tool never truncates or silently drops source material.

| Input | Limit |
| --- | ---: |
| Spec JSON | 64 KiB |
| Packet JSON | 4 MiB |
| One source file | 1 MiB |
| Unique referenced source files combined | 4 MiB |
| Source references | 64 |
| Task text | 8 KiB |
| Repository-relative path | 400 bytes |

These bounds limit file and artifact I/O. They do not select evidence or define
model context capacity. Required material is never inferred from repository
instructions: the caller must list it. Source completeness and the consumer's
independent correctness criteria remain consumer-owned.

The library exposes `read_spec`, `read_packet`, `build`, `verify`, and
`Packet::context_chunks()` so a caller can project the stored excerpts into
Redshirt's existing context chunks. `context_chunks()` only projects stored
content; it does not verify freshness. Run `verify` successfully before current
reuse. This does not change context-comparison v3: it retains every supplied
excerpt that passes model-capacity admission. A complete comparison still needs
its own manifest, cases, and independent oracle. Packet verification does not
provide that grading key or establish answer quality.

Linux verification opens each path component without following symlinks. Other
platforms use metadata checks before opening; that fallback does not prevent a
concurrent path swap. The local and hosted acceptance checks exercise Linux.

## Next measurements

The next useful step is to reuse the existing BM25 baseline to order candidate
chunks for a concrete task, keeping the complete packet available and reviewing
source completeness independently.
If Jev relevance ranking is added, first run it in shadow against the preserved
complete packet. Any live ranking comparison needs its own bounded allowance.
Evaluate task outcomes with consumer-owned checks before changing how an agent
uses evidence. These packets add no dispatch, background worker, or hidden-context
hook.
