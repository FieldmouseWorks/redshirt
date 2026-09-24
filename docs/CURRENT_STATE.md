# Current state

What runs today and what's open. Git history and closed issues/PRs hold the details.

## Runs

| Surface | Status | Docs |
| --- | --- | --- |
| Rust controller + adapter process | Owns full episodes: budgets, cancellation, evidence, concrete replay (Python/Rust both directions) | [RUST-ADAPTER](RUST-ADAPTER.md) |
| External JSON sessions (`--stdio`) | Unchanged Python client and wire contract under the Rust owner; role-filtered views | [INTERACTION](INTERACTION.md) |
| Host-configured menu bounds | Trusted hosts can widen action menus; defaults unchanged | [RUST-ADAPTER](RUST-ADAPTER.md#host-configured-menu-bounds) |
| Native Jev provider (`jev-http`) | Pinned action selection, batched questions, per-call receipts, confidence abstention | [JEV-RUST](JEV-RUST.md) |
| Fixed JSON selectors (`choice-http`) | Direct candidate selection profiles; model-free replay | [CHOICE](CHOICE.md) |
| Comparison runner | Consumer baselines vs. optional provider over fixed cases, campaign reservations | [COMPARISON](COMPARISON.md) |
| Context comparison (experimental) | Deterministic evidence ordering vs. Jev relevance, same diagnostic model both arms | [CONTEXT-COMPARISON](CONTEXT-COMPARISON.md) |
| Source-backed evidence packets | Caller-selected excerpts with git refs and freshness checks | [EVIDENCE-PACKETS](EVIDENCE-PACKETS.md) |
| Shadow ranking | BM25 vs. one Jev relevance batch per packet; offline prep/mock/replay | [SHADOW-RANKING](SHADOW-RANKING.md) |
| Python runner | Transition baseline for existing callers; attach mode supported | [BROWSER-SLICE](BROWSER-SLICE.md), [JEV](JEV.md) |

Consumers: LoK-web removed its old browser adapter along with its JS engine; an adapter against
its Rust server contract is still to be written there. Conary's `conary-test` harness has its own
explorer (Conary PR #1051, open) and hasn't migrated to this runtime.

## Limits

- Runtime support doesn't establish model usefulness; no provider quality claim is made.
- Attached sessions can't claim reset or replay.
- Public CI has no browser gate; consumer integration is verified in the consumer.

## Open

- [#42](https://github.com/FieldmouseWorks/redshirt/issues/42): the shadow-ranking live run
  (4 Jev calls, $0.02 cap) is blocked on a provisioned Jev credential. Code is merged.
- [#26](https://github.com/FieldmouseWorks/redshirt/issues/26): investigate a live Jev Choice
  disagreement with its reported maximum.
