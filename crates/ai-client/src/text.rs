//! What the chat adapters share: the `text.complete` spec, messages and options.

use craft_ai_core::model::InputRole;
use craft_ai_core::{AiError, CapabilityId, CapabilitySpec, Features, Payload, Request};
use serde_json::{Map, Value, json};

/// Most context messages one chat request may carry.
const MAX_CONTEXT: u32 = 256;

/// The capability every chat adapter offers.
pub(crate) fn text_complete() -> Result<CapabilityId, AiError> {
    CapabilityId::parse("text.complete").map_err(|e| AiError::new(craft_ai_core::ErrorKind::Internal, e.to_string()))
}

/// `text.complete` as the chat adapters run it: one prompt (required), an optional system
/// message, earlier turns as `context` inputs, one output, streaming available.
pub fn chat_spec() -> Option<CapabilitySpec> {
    let capability = text_complete().ok()?;
    Some(
        CapabilitySpec::new(capability)
            .accepting(InputRole::System, 1)
            .accepting(InputRole::Context, MAX_CONTEXT)
            .accepting(InputRole::Prompt, 1)
            .requiring(InputRole::Prompt)
            .with_features(Features { streaming: true, ..Features::default() }),
    )
}

/// The chat messages for a request: the system message first, then context turns in order
/// (alternating user and assistant, starting with the user), then the prompt as the user.
/// Only text inputs can be sent; anything else is `Unsupported`.
pub(crate) fn messages(request: &Request) -> Result<Vec<Value>, AiError> {
    let text_of = |payload: &Payload| match payload {
        Payload::Text { text } => Ok(text.clone()),
        _ => Err(AiError::unsupported("this adapter sends text only; images and files are not supported here")),
    };
    let mut out = Vec::new();
    for input in request.inputs.iter().filter(|i| i.role == InputRole::System) {
        out.push(json!({ "role": "system", "content": text_of(&input.payload)? }));
    }
    for (turn, input) in request.inputs.iter().filter(|i| i.role == InputRole::Context).enumerate() {
        let role = if turn % 2 == 0 { "user" } else { "assistant" };
        out.push(json!({ "role": role, "content": text_of(&input.payload)? }));
    }
    for input in request.inputs.iter().filter(|i| i.role == InputRole::Prompt) {
        out.push(json!({ "role": "user", "content": text_of(&input.payload)? }));
    }
    Ok(out)
}

/// The options a chat request may carry, by their provider-neutral names.
pub(crate) const CHAT_PARAMS: &[&str] = &["temperature", "top_p", "max_tokens", "seed", "stop", "presence_penalty", "frequency_penalty"];

/// Checks that every param is one the chat adapters understand, so a typo is an error rather
/// than silently ignored.
pub(crate) fn check_params(params: &Map<String, Value>, allowed: &[&str]) -> Result<(), AiError> {
    match params.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(unknown) => Err(AiError::invalid(format!("the option '{unknown}' is not supported here; use one of: {}", allowed.join(", ")))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Request {
        Request::new(text_complete().unwrap())
            .with_input(InputRole::Prompt, Payload::text("and now?"))
            .with_input(InputRole::Context, Payload::text("hi"))
            .with_input(InputRole::Context, Payload::text("hello!"))
            .with_input(InputRole::System, Payload::text("be brief"))
    }

    #[test]
    fn messages_are_ordered_system_context_prompt() {
        let m = messages(&request()).unwrap();
        let roles: Vec<&str> = m.iter().filter_map(|v| v["role"].as_str()).collect();
        assert_eq!(roles, ["system", "user", "assistant", "user"]);
        assert_eq!(m[3]["content"], "and now?");
    }

    #[test]
    fn non_text_inputs_and_unknown_params_are_refused() {
        let image = Request::new(text_complete().unwrap()).with_input(InputRole::Prompt, Payload::inline("image/png", vec![1]).unwrap());
        assert_eq!(messages(&image).unwrap_err().kind, craft_ai_core::ErrorKind::Unsupported);
        let mut params = Map::new();
        params.insert("temprature".into(), json!(1));
        assert!(check_params(&params, CHAT_PARAMS).is_err());
        params.clear();
        params.insert("temperature".into(), json!(1));
        assert!(check_params(&params, CHAT_PARAMS).is_ok());
    }

    #[test]
    fn the_spec_validates_requests() {
        let spec = chat_spec().unwrap();
        assert!(request().validate(&spec, &Default::default()).is_ok());
        let no_prompt = Request::new(text_complete().unwrap());
        assert!(no_prompt.validate(&spec, &Default::default()).is_err());
    }
}
