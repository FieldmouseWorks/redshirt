# Architecture decision: Rust core, flexible integrations

Accepted by the owner on2026-09-20: Rust where it fits the durable systems,
with other languages retained for useful integration flexibility. This supersedes
any assumption that the existing Python proof chooses the permanent core language.
The first [Rust adapter-boundary proof](RUST-ADAPTER.md) is implemented. Existing
Python callers and Conary's pilot have not all been migrated.

## Ownership and implementation boundary

Redshirt's shared controller, budgets, cancellation, provider contracts, evidence
and concrete replay belong in a Rust library with a small executable interface.
Project-specific state, rules, role/capability policy, actor bindings and independent
checks remain in project adapters. Provider choice never grants permissions.
Public Redshirt remains MIT and contains no private project source or assets.

Keep versioned structured contracts at integration boundaries. A Rust application
can use the library; a browser, Python adapter or another model host can use a
process/API boundary. Do not require an integration to adopt Rust merely to send
observations or choose permitted actions. Keep optional providers replaceable,
and preserve model-free execution and replay.

Python remains appropriate for demonstrated tooling and browser integrations.
The existing Python runner is the verified transition baseline, not a second
permanent controller. Conary's existing Rust pilot is also a baseline; it still
contains package action/fact types and a package-specific oracle, so extraction
must preserve its proofs while moving those responsibilities behind adapter traits.

## First migration proof

1. Extract only demonstrated common controller/provider/evidence behavior into a
   Rust core, retaining Conary's provenance and MIT notices.
2. Introduce a narrow adapter interface for setup, observation/candidates,
   verification, execution, independent evaluation and close. Run the existing
   browser adapter under the Rust owner without porting the browser tooling.
3. Match budgets, stale/busy refusal, cancellation, mandatory final checks,
   failed/uncertain-input evidence, attached-session restrictions and concrete
   model-free replay. Only then cut over a caller and retire its old controller.

The [current JSON interface](INTERACTION.md) is selector-facing. Connecting a Rust
controller to it while leaving a Python controller underneath would not satisfy
the single-owner requirement. Preserve the external contract as ownership moves;
do not nest controllers or rewrite every integration at once.

A single resettable browser episode with identical independently checked outcomes
and zero-provider replay is the first integration acceptance check. Use synthetic
failure/cancellation cases alongside it. No live model call or performance claim
is required to establish language parity.

The Rust process path owns its complete episode. Its Python worker only hosts
adapter methods and optional provider transport. The existing JSON decision
contract and unchanged Python client now run against that same owner, including
finite fractional observations and v1 replay. This path requires zero captures;
other callers retain their explicit baseline until their cutover checks pass.
