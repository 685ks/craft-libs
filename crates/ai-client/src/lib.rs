#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

//! HTTP adapters for the Craft AI contract ([`craft_ai_core::Provider`]).
//!
//! | Adapter | Talks to | Capabilities |
//! |---|---|---|
//! | [`OpenAiCompatible`] | OpenAI, LM Studio, vLLM and other `/v1/chat/completions` servers | `text.complete`, streamed over SSE |
//! | [`Ollama`] | a local Ollama server (`/api/chat`) | `text.complete`, streamed as NDJSON |
//! | [`Bfl`] | Black Forest Labs (`api.bfl.ai`) | `image.generate`, and `image.edit` for Kontext models, by job polling |
//!
//! Every adapter follows the contract: `submit` validates the request against the model's spec
//! before sending anything and starts a job on a worker thread; `poll` returns the state and
//! the events since the last poll; `cancel` stops the job at once (a request already on the wire
//! is abandoned, and its result is dropped).
//!
//! Network behaviour is set by [`HttpConfig`]: connect and request timeouts, a cap on response
//! sizes, and [`RetryPolicy`] (exponential backoff, honouring `Retry-After`). Requests that may
//! already have reached the server are only retried when the server says it did not process
//! them (429, 503), so a paid image is never ordered twice.
//!
//! Keys come from the user's keychain through [`craft_secrets`]: build an adapter with
//! `from_connection(connection, store, config)` and the connection's
//! [`SecretRef`](craft_ai_core::SecretRef) is resolved once, into an [`ApiKey`] that is never
//! printed and is scrubbed from every error message.
//!
//! This crate is desktop only (it uses sockets and threads); `craft-ai-core` stays the
//! wasm-safe contract.

mod auth;
mod bfl;
mod http;
mod jobs;
mod ollama;
mod openai;
mod sse;
mod text;

pub use auth::{ApiKey, KeyHeader, resolve_key};
pub use bfl::{BFL_DEFAULT_ENDPOINT, Bfl, bfl_models};
pub use http::{HttpConfig, RetryPolicy};
pub use ollama::{OLLAMA_DEFAULT_ENDPOINT, Ollama};
pub use openai::{OPENAI_DEFAULT_ENDPOINT, OpenAiCompatible};
pub use sse::{SseEvent, SseParser};
pub use text::chat_spec;
