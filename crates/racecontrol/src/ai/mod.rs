//! Central AI service for RaceControl.
//!
//! All AI calls (chat, crash analysis, pattern detection) route through this module.
//! Priority: Claude CLI → OpenRouter (MMA-class cloud models) → Ollama (venue, with learned context) → Anthropic API.
//! Automatically logs Claude CLI responses as training pairs for Ollama to learn from.

mod context;
mod learning;
mod providers;

// Re-export public API
pub use context::{
    build_customer_prompt, build_staff_prompt, gather_business_context, gather_customer_context,
};
pub use learning::{extract_keywords_pub, sanitize_for_prompt};
pub use providers::{query_anthropic, query_claude_cli, query_ollama, query_openrouter};

use serde_json::Value;
use sqlx::SqlitePool;

use crate::config::AiDebuggerConfig;
use learning::{
    build_enhanced_messages, extract_user_query, find_similar_pairs, log_training_pair,
    messages_to_prompt,
};
use providers::read_openrouter_key;

/// Try Claude CLI → Ollama (venue, with learned context) → Anthropic API. Returns (response, model_used).
/// When `db` is provided, automatically logs responses as training pairs for future learning.
pub async fn query_ai(
    config: &AiDebuggerConfig,
    messages: &[Value],
    db: Option<&SqlitePool>,
    source: Option<&str>,
) -> anyhow::Result<(String, String)> {
    let user_query = extract_user_query(messages);

    // 1. Primary: Claude CLI
    if config.claude_cli_enabled {
        let prompt = messages_to_prompt(messages);
        match query_claude_cli(&prompt, config.claude_cli_timeout_secs).await {
            Ok(reply) => {
                // Log this Q&A pair for future learning
                if let Some(db) = db {
                    log_training_pair(
                        db,
                        &user_query,
                        &reply,
                        source.unwrap_or("unknown"),
                        "claude-cli",
                    )
                    .await;
                }
                return Ok((reply, "claude-cli".to_string()));
            }
            Err(e) => {
                tracing::warn!("Claude CLI failed: {}. Trying Ollama...", e);
            }
        }
    }

    // 2. OpenRouter (MMA-class cloud models — higher quality than Ollama)
    if read_openrouter_key().is_some() {
        match query_openrouter(&config.openrouter_model, messages).await {
            Ok(reply) => {
                if let Some(db) = db {
                    log_training_pair(
                        db,
                        &user_query,
                        &reply,
                        source.unwrap_or("unknown"),
                        &format!("openrouter/{}", config.openrouter_model),
                    )
                    .await;
                }
                return Ok((reply, format!("openrouter/{}", config.openrouter_model)));
            }
            Err(e) => {
                tracing::warn!("OpenRouter failed: {}. Trying Ollama...", e);
            }
        }
    }

    // 3. Fallback: Ollama (venue-local, with learned context from training pairs)
    {
        let few_shot = if let Some(db) = db {
            find_similar_pairs(db, &user_query, 3).await
        } else {
            vec![]
        };
        let enhanced = build_enhanced_messages(messages, &few_shot);

        match query_ollama(&config.ollama_url, &config.ollama_model, &enhanced).await {
            Ok(reply) => {
                // Increment use_count on training pairs we used
                if let Some(db) = db {
                    for pair in &few_shot {
                        let _ = sqlx::query(
                            "UPDATE ai_training_pairs SET use_count = use_count + 1 WHERE id = ?",
                        )
                        .bind(&pair.id)
                        .execute(db)
                        .await;
                    }
                }

                tracing::info!(
                    "AI query answered by Ollama (with {} examples)",
                    few_shot.len()
                );
                return Ok((reply, format!("ollama/{}", config.ollama_model)));
            }
            Err(e) => {
                tracing::warn!("Ollama failed: {}. Trying Anthropic API...", e);
            }
        }
    }

    // 4. Final fallback: Anthropic API
    if let Some(api_key) = &config.anthropic_api_key {
        let reply = query_anthropic(api_key, &config.anthropic_model, messages).await?;
        if let Some(db) = db {
            log_training_pair(
                db,
                &user_query,
                &reply,
                source.unwrap_or("unknown"),
                &format!("anthropic/{}", config.anthropic_model),
            )
            .await;
        }
        Ok((reply, format!("anthropic/{}", config.anthropic_model)))
    } else {
        anyhow::bail!("All AI providers failed (Claude CLI, OpenRouter, Ollama, Anthropic API)")
    }
}
