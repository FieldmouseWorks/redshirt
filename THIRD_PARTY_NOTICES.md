# Third-party provenance

The probability-total acceptance policy in `redshirt/jev.py` follows the Conary
pilot at commit `4278a9202f2bc87f58d54547f5c03e37cf14d26f`,
[`apps/conary-test/src/explorer/jev/choice.rs`](https://github.com/FieldmouseWorks/Conary/blob/4278a9202f2bc87f58d54547f5c03e37cf14d26f/apps/conary-test/src/explorer/jev/choice.rs).
The Rust controller in `src/controller.rs` also adapts the admission, dispatch
revalidation, uncertain-attempt accounting and mandatory-finalization invariants
demonstrated by `controller.rs` and `contract.rs` at that same Conary revision.
The existing standalone Python runner supplies the v1 external-adapter/replay
contract. No Conary package action types, Oracle or fixtures are imported.
Conary's Rust integration remains unchanged. The tolerance is explicit consumer
policy; values are retained without normalization. Applicable MIT notice:

MIT License

Copyright (c) 2025-2026 Conary Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
