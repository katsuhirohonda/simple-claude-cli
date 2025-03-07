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

    let anthropic_api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY is not set");
    let anthropic_client =
        AnthropicClient::new::<MessageError>(anthropic_api_key, "2023-06-01").unwrap();

    let meta_prompt = format!("You are an excellent software engineer.");
    let system_message = meta_prompt;

    let body = CreateMessageParams::new(RequiredMessageParams {
        model: "claude-3-5-haiku-20241022".to_string(),
        messages: vec![Message::new_text(Role::User, task_description)],
        max_tokens: 1024,
    })
    .with_stream(true)
    .with_system(system_message);

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
                            //info!("Received event: {:?}", event);
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
