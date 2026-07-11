//! On-device text generation from a user-supplied GGUF model, via `candle`
//! (pure-Rust, CPU-only — no C++ toolchain or GPU driver requirement, unlike
//! llama.cpp bindings). This is the "nothing uploaded" alternative to the
//! OpenAI/Claude providers in [`crate::core::llm`]: prompts, screenshots, and
//! accessibility-tree context never leave the machine.
//!
//! Ghost does not ship or download model weights. The user points
//! `AISettings.local_model_path` at a `.gguf` file and
//! `local_tokenizer_path` at its matching `tokenizer.json` (both are
//! standard files published alongside quantized models, e.g.
//! `Meta-Llama-3-8B-Instruct.Q4_K_M.gguf`); nothing loads until both are
//! set. Compiled only behind the `experimental` Cargo feature — see
//! `core/mod.rs` and the `experimental` feature entry in `Cargo.toml`.
//!
//! Architecture support comes from `candle_transformers::models::quantized_llama`,
//! which reads the GGUF `general.architecture` metadata to pick the right
//! RoPE convention, covering the Llama family (Llama 2/3, Mistral) and,
//! per its own architecture table, Phi-3 and Qwen2 GGUF files as well —
//! matching the "Llama-3-8B or Phi-3" ask without a second code path.

use crate::core::events::{ElementInfo, InputEvent};
use crate::core::llm::{parse_events_json, LLMProvider};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_llama::ModelWeights;
use std::fs::File;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

/// How many trailing tokens the repeat penalty looks back over.
const REPEAT_LAST_N: usize = 64;
/// Repeat penalty strength. 1.0 disables it; candle's own examples default
/// to ~1.1 to keep small quantized models from looping.
const REPEAT_PENALTY: f32 = 1.1;
/// Fixed seed: suggestions should be reproducible for the same prompt and
/// workflow context, not vary run to run.
const SAMPLING_SEED: u64 = 299_792_458;
const TOP_P: f64 = 0.9;

/// A loaded local model, ready to generate. Construction (`load`) does the
/// expensive part — reading the GGUF file and building the tokenizer — once;
/// `generate_workflow` reuses it for every call.
///
/// Model and tokenizer are `Arc`-wrapped (rather than borrowed) so a call can
/// hand cloned handles into `tokio::task::spawn_blocking`, which requires a
/// `'static` closure — candle's CPU forward pass is synchronous and must not
/// run on the async executor's own worker thread.
pub struct LocalModelProvider {
    model: Arc<Mutex<ModelWeights>>,
    tokenizer: Arc<Tokenizer>,
    eos_token_id: u32,
    device: Device,
    max_tokens: usize,
    temperature: f64,
}

impl LocalModelProvider {
    /// Load a GGUF model and its tokenizer from disk. CPU-only (`Device::Cpu`).
    pub fn load(
        model_path: &str,
        tokenizer_path: &str,
        max_tokens: usize,
        temperature: f32,
    ) -> anyhow::Result<Self> {
        let device = Device::Cpu;

        let mut file = File::open(model_path)
            .map_err(|e| anyhow::anyhow!("cannot open local model at {model_path}: {e}"))?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| anyhow::anyhow!("cannot parse GGUF file {model_path}: {e}"))?;
        let eos_token_id = content
            .metadata
            .get("tokenizer.ggml.eos_token_id")
            .and_then(|v| v.to_u32().ok())
            .ok_or_else(|| {
                anyhow::anyhow!("{model_path}: GGUF metadata has no tokenizer.ggml.eos_token_id")
            })?;
        let model = ModelWeights::from_gguf(content, &mut file, &device)
            .map_err(|e| anyhow::anyhow!("cannot load model weights from {model_path}: {e}"))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("cannot load tokenizer at {tokenizer_path}: {e}"))?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            tokenizer: Arc::new(tokenizer),
            eos_token_id,
            device,
            max_tokens: max_tokens.max(1),
            temperature: temperature.max(0.0) as f64,
        })
    }
}

/// Autoregressive decoding loop: prime the KV cache with the full prompt in
/// one forward pass, then decode one token at a time until EOS or
/// `max_tokens`. Synchronous — candle's CPU forward pass is not async — so
/// callers run this via `spawn_blocking`; a free function (rather than a
/// `&self` method) so it only needs the `Arc`-cloned pieces it touches, with
/// no borrow of the provider to keep alive across the blocking call.
fn generate_blocking(
    model: &Mutex<ModelWeights>,
    tokenizer: &Tokenizer,
    device: &Device,
    eos_token_id: u32,
    max_tokens: usize,
    temperature: f64,
    prompt: &str,
) -> anyhow::Result<String> {
    let encoding = tokenizer
        .encode(prompt, true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode failed: {e}"))?;
    let mut context = encoding.get_ids().to_vec();
    if context.is_empty() {
        return Ok(String::new());
    }

    let mut model = model
        .lock()
        .map_err(|_| anyhow::anyhow!("local model lock poisoned"))?;
    // Independent from any prior call — this prompt owns a fresh context.
    model.clear_kv_cache();

    let mut logits_processor = LogitsProcessor::new(SAMPLING_SEED, Some(temperature), Some(TOP_P));
    let mut generated = Vec::new();
    let mut index_pos = 0usize;
    let mut next_input = context.clone();

    for step in 0..max_tokens {
        let input = Tensor::new(next_input.as_slice(), device)?.unsqueeze(0)?;
        let logits = model.forward(&input, index_pos)?.squeeze(0)?;
        let logits = if REPEAT_PENALTY == 1.0 {
            logits
        } else {
            let start = context.len().saturating_sub(REPEAT_LAST_N);
            candle_transformers::utils::apply_repeat_penalty(
                &logits,
                REPEAT_PENALTY,
                &context[start..],
            )?
        };
        index_pos += next_input.len();

        let next_token = logits_processor.sample(&logits)?;
        context.push(next_token);
        if next_token == eos_token_id {
            break;
        }
        generated.push(next_token);
        if step + 1 == max_tokens {
            break;
        }
        next_input = vec![next_token];
    }

    tokenizer
        .decode(&generated, true)
        .map_err(|e| anyhow::anyhow!("tokenizer decode failed: {e}"))
}

/// Wraps the prompt in the Llama-3-Instruct chat template. Models with a
/// different template (e.g. a non-Llama-family Phi-3 GGUF using its own
/// tokenizer conventions) will still run — `quantized_llama` reads the
/// architecture from GGUF metadata independently of this formatting — but
/// may follow the JSON-schema instruction less reliably without their native
/// template. Documented follow-up rather than a blocker: getting *a*
/// working local provider shipped first matters more than covering every
/// chat template up front.
fn llama3_instruct_prompt(system_prompt: &str, user_prompt: &str) -> String {
    format!(
        "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{system_prompt}<|eot_id|>\
         <|start_header_id|>user<|end_header_id|>\n\n{user_prompt}<|eot_id|>\
         <|start_header_id|>assistant<|end_header_id|>\n\n"
    )
}

#[async_trait::async_trait]
impl LLMProvider for LocalModelProvider {
    async fn generate_workflow(
        &self,
        prompt: &str,
        _screenshot: Option<&[u8]>,
        ax_tree: Option<&str>,
        element_context: &[ElementInfo],
    ) -> anyhow::Result<Vec<InputEvent>> {
        let ax_context = ax_tree
            .map(|t| crate::core::compress::compress_to_budget(t, 2000).0)
            .unwrap_or_else(|| "no context".to_string());

        let system_prompt = format!(
            "You are an AI automation assistant. Convert natural language commands into structured input events.\
            \n\nAvailable UI elements: {:?}\
            \n\nAccessibility tree context: {}\
            \n\nOutput ONLY valid JSON matching this schema: {{\"events\": [INPUT_EVENTS]}}\
            \nWhere each INPUT_EVENT is one of:\
            \n- {{\"MouseClick\": {{\"x\": int, \"y\": int, \"button\": int, \"element\": {{\"role\": str, \"name\": str, \"app\": str}}}}}}\
            \n- {{\"Key\": {{\"code\": int, \"chars\": str, \"modifiers\": int, \"action\": \"Down\"| \"Up\"}}}}\
            \n- {{\"Delay\": {{\"ms\": int}}}}\
            \n- {{ \"Scroll\": {{\"dx\": int, \"dy\": int}}}}\
            \nSet optional fields to null. Keep events array minimal but complete. Output nothing but the JSON.",
            element_context.iter().take(10).collect::<Vec<_>>(),
            ax_context
        );
        let full_prompt = llama3_instruct_prompt(&system_prompt, prompt);

        // candle's forward pass is blocking CPU work; keep it off the async
        // executor. Clone the `Arc`s (cheap) rather than borrow `self`, since
        // `spawn_blocking` requires a `'static` closure.
        let model = Arc::clone(&self.model);
        let tokenizer = Arc::clone(&self.tokenizer);
        let device = self.device.clone();
        let eos_token_id = self.eos_token_id;
        let max_tokens = self.max_tokens;
        let temperature = self.temperature;
        let text = tokio::task::spawn_blocking(move || {
            generate_blocking(
                &model,
                &tokenizer,
                &device,
                eos_token_id,
                max_tokens,
                temperature,
                &full_prompt,
            )
        })
        .await
        .map_err(|e| anyhow::anyhow!("local model generation task panicked: {e}"))??;

        // A local model is far more likely than GPT-4/Claude to produce
        // malformed JSON. This is a suggestion-only feature (see
        // docs/ "Ghost Intelligence"), so a parse failure degrades to "no
        // suggestions" rather than a hard error surfaced to the user.
        match parse_events_json(&text) {
            Ok(events) => Ok(events),
            Err(e) => {
                tracing::warn!("local model output was not valid events JSON: {e}");
                Ok(Vec::new())
            }
        }
    }

    fn name(&self) -> &'static str {
        "Local Model"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llama3_prompt_wraps_system_and_user_turns_with_header_tags() {
        let prompt = llama3_instruct_prompt("be helpful", "click submit");
        assert!(prompt.starts_with("<|begin_of_text|><|start_header_id|>system"));
        assert!(prompt.contains("be helpful<|eot_id|>"));
        assert!(
            prompt.contains("<|start_header_id|>user<|end_header_id|>\n\nclick submit<|eot_id|>")
        );
        assert!(prompt.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));
    }
}
