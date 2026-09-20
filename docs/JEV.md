# Optional Jev Choice provider

Install the optional transport in the invoking application's virtual environment:

```sh
python -m pip install '/path/to/redshirt[jev]'
```

```python
import os
from redshirt import Limits, run
from redshirt.jev import Jev

provider = Jev.live(os.environ['TYPESAFE_API_KEY'], request_limit=6)
report = await run(adapter, output, provider=provider, limits=Limits(requests=6))
```

This explicit constructor enables the pinned HTTPS endpoint
`https://api.typesafe.ai/v1/systemone` and model `jev-1.13.0`. Default selectors and
replay need neither the optional dependency nor a credential. Applications own
credential provisioning and authorization for their data and spend.

The model receives only the runner's audited observation, complete current
candidate descriptions, and remaining input budget. There is one Choice question;
no candidate pruning, chained model conversation, image input, generated commands
or model-supplied evaluator. The controller accepts up to 96 adapter candidates
plus stop, while retaining its existing byte/input/time/evidence bounds.

Each provider permits at most 12 configured calls (default six), one in flight,
with a five-second absolute deadline including transport and body reads. There
are no retries, fallback models, redirects or inherited environment proxies.
Requests and responses are each bounded to 16 KiB. HTTP errors, cancellation,
malformed JSON, duplicate keys, model drift, invalid usage, missing probabilities,
out-of-range values and nonmaximum choices fail closed.

The sum tolerance is an explicit consumer policy: `abs(sum - 1) <= .01 + 1e-12`.
Accepted probabilities retain their original values; no normalization, substituted
choice or confidence threshold is applied. The receipt distinguishes exact and
accepted approximate totals. This follows the Conary pilot with its retained
[MIT notice and provenance](../THIRD_PARTY_NOTICES.md); the Rust integration has
not been migrated.

The shared runner drains one bounded `provider_receipt` after each selection,
including failure or cancellation. It retains the exact request and digest,
bounded response text, status, latency, validation result and successful usage.
Headers and exception text are omitted; known reflected credentials are redacted.
Estimated input-token cost is distinct from billing, which remains unknown.
Uncertain dispatches consume a call and full context reservation.

At the documented input price of $0.042/million tokens and free output, reserving
65,536 input tokens per call yields $0.016515072 for six calls, or $0.033030144 for
twelve. This is a conservative application reservation at the inspected price,
not a guarantee about billing or future pricing. Official docs checked 2026-09-20:
[API](https://docs.typesafe.ai/api), [models/pricing](https://docs.typesafe.ai/models),
[Choice](https://docs.typesafe.ai/primitives/choice),
[confidence](https://docs.typesafe.ai/confidence). The official
[TypeSafe skill](https://github.com/typesafe-ai/skills/blob/65a39f393687675ce170e6094757de20370365b9/skills/typesafe-ai/SKILL.md)
informed the typed, candidate-only boundary.

For network-free tests, inject an async transport into `Jev(transport)`. It takes
request JSON bytes and returns `(status, response_bytes)`. These tests preserve
the same encoding and validation but are explicitly labelled `jev-mock`; scripted
mock answers establish no model ability. The optional HTTPX transport test also
uses an in-memory transport and sends no live requests.
