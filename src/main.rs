use anthropic_ai_sdk::clients::AnthropicClient;
use anthropic_ai_sdk::types::message::StreamEvent;
use anthropic_ai_sdk::types::message::{
    ContentBlockDelta, CreateMessageParams, Message, MessageClient, MessageError,
    RequiredMessageParams, Role,
};
use colored::*;
use futures_util::StreamExt;
use tracing::error;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(false)
        .with_level(true)
        .try_init()
        .expect("Failed to initialize logger");

    // Read API key, model and persona from environment variables
    let anthropic_api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY is not set");
    let model = std::env::var("CLAUDE_MODEL").unwrap_or("claude-3-haiku-20240229".to_string());

    // Allow user to set a custom system prompt via environment variable
    let default_system_prompt = "You are a helpful assistant.".to_string();
    let system_prompt = std::env::var("CLAUDE_SYSTEM_PROMPT").unwrap_or(default_system_prompt);

    // Display predefined personas that users can choose with --persona flag
    let persona = match std::env::var("CLAUDE_PERSONA").ok().as_deref() {
        Some("engineer") => "You are an excellent software engineer.".to_string(),
        Some("writer") => "You are a creative writer with excellent language skills.".to_string(),
        Some("scientist") => {
            "You are a scientist with expertise in various scientific fields.".to_string()
        }
        Some("teacher") => "You are a patient teacher who explains concepts clearly.".to_string(),
        Some("chef") => {
            "You are a professional chef with extensive culinary knowledge.".to_string()
        }
        Some("therapist") => "You are a compassionate therapist who listens carefully.".to_string(),
        Some(custom) => custom.to_string(),
        None => system_prompt,
    };

    print!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
    println!("    {}\n", "How can I help you today?".bold());

    let mut task_description = String::new();
    std::io::stdin()
        .read_line(&mut task_description)
        .expect("Failed to read line");
    let task_description = task_description.trim().to_string();

    if task_description.is_empty() {
        println!("{} {}", ">>>".red(), "[Error]:".red().bold());
        println!(
            "    {}",
            "I need a task description to get started. Please try again.".bright_white()
        );
        std::process::exit(1);
    }

    let anthropic_client =
        AnthropicClient::new::<MessageError>(anthropic_api_key, "2023-06-01").unwrap();

    let body = CreateMessageParams::new(RequiredMessageParams {
        model: model,
        messages: vec![Message::new_text(Role::User, task_description)],
        max_tokens: 1024,
    })
    .with_stream(true)
    .with_system(persona.to_string());

    println!(
        "\n{} {}",
        ">>>".bright_blue(),
        "[Assistant]:".green().bold()
    );

    match anthropic_client.create_message_streaming(&body).await {
        Ok(mut stream) => {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(event) => {
                        if let StreamEvent::ContentBlockDelta { index: _, delta } = event {
                            if let ContentBlockDelta::TextDelta { text } = delta {
                                print!("{}", text);
                            }
                        }
                    }
                    Err(e) => error!("Stream error: {}", e),
                }
            }
        }
        Err(e) => {
            error!("Error: {}", e);
        }
    }

    Ok(())
}
