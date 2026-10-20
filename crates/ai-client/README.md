# craft-ai-client

HTTP adapters that implement `craft_ai_core::Provider`. Desktop only: it uses blocking HTTP
(`ureq` with rustls) on worker threads.

- `OpenAiCompatible`: `/v1/models` and `/v1/chat/completions`, for OpenAI, LM Studio, vLLM and
  any other server that speaks the same API. Streams Server-Sent Events into `JobEvent::Delta`s.
- `Ollama`: `/api/tags` and `/api/chat`, streaming newline-delimited JSON.
- `Bfl`: Black Forest Labs (FLUX) image jobs. It submits, polls the `polling_url` the service
  returns, and downloads the sample. The key goes in `x-key` and is sent only to the API's own
  host or site, never to the CDN that serves the image.

Every provider keeps its jobs in a table, so `submit` returns at once and `poll` drains events.
`cancel` takes effect immediately, including in the middle of a stream or a retry wait.

`HttpConfig` sets the connect, request and stream timeouts, a response size cap and the retry
policy: exponential backoff, doubling up to a cap, that honours `Retry-After`. GETs
are retried on network errors and 5xx. POSTs are retried only on 429/503 or when the connection
never opened, so a generation is never billed twice.

Keys come from a `Connection`'s `SecretRef` through any `craft_secrets::SecretStore`
(`resolve_key`). They are held as `SecretString`, sent only in the configured header, and
redacted from every error message. Plain `http://` is accepted only for loopback endpoints.

Tests run against a local mock HTTP server, with no real network.
