use anthropic_ai_sdk::clients::AnthropicClient;
use anthropic_ai_sdk::types::message::ContentBlock;
use colored::*;
use mcp_sdk_rs::types::Content::Text;
use mcp_sdk_rs::types::TextContent;
use models::FeedbackType;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::{
    FeedbackInfo, MessageDocument, SessionDocument, SessionMetadata, TaskState, ToolInfo,
};
use crate::operation::SearchType;
use crate::operation::search_similar_sessions;
use anthropic_ai_sdk::types::message::{
    CreateMessageParams, Message, MessageClient, MessageError, RequiredMessageParams, Role, Tool,
};
use chrono::Utc;
use mcp_sdk_rs::client::index::Client;
use mcp_sdk_rs::client::index::ClientOptions;
use mcp_sdk_rs::client::stdio::{StdioClientTransport, StdioMode, StdioServerParameters};
use mcp_sdk_rs::error::{ErrorCode, McpError};
use mcp_sdk_rs::types::{CallToolRequest, CallToolResult, ClientCapabilities, Implementation};
use models::{ContinueActionImproved, MCPMultiClient, MCPServersConfig};

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

use crate::operation::add_comment;
use crate::operation::add_rag_feedback;
use crate::operation::get_next_action_with_options;

use crate::parser::parse_embedded_tooluse_in_text;
use crate::settings::load_env_config;
use crate::settings::set_mcp_servers_config;
use tracing::info;

trait IntoAnthropicTool {
    fn into_anthropic_tools(self) -> Vec<anthropic_ai_sdk::types::message::Tool>;
}

impl IntoAnthropicTool for Vec<mcp_sdk_rs::types::Tool> {
    fn into_anthropic_tools(self) -> Vec<anthropic_ai_sdk::types::message::Tool> {
        self.into_iter()
            .map(|t| anthropic_ai_sdk::types::message::Tool {
                name: t.name,
                description: t.description,
                input_schema: serde_json::to_value(t.input_schema).unwrap_or_default(),
            })
            .collect()
    }
}

/// run the agent
///
#[tokio::main]
async fn main() -> Result<(), McpError> {
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

    load_env_config();
    let config = set_mcp_servers_config();

    println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
    println!("    {}", "How can I help you today? If your task includes !!, it means you need to search similar tasks from the past.".bright_white());

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

    let multi_client = MCPMultiClient::new(config).await;
    let tools = multi_client.initialize().await?;

    let anthropic_api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY is not set");
    let anthropic_client =
        AnthropicClient::new::<MessageError>(anthropic_api_key, "2023-06-01").unwrap();

    // メモ：推論手法の説明
    //    Chain-of-Thought (CoT)
    //    問題を段階的に解くため、途中の思考過程を順に出力する手法。
    //
    //    Zero-shot Chain-of-Thought
    //    事前例示なしで、モデル自体に段階的推論を促す方法。
    //
    //    Automatic Chain-of-Thought (Auto-CoT)
    //    モデルが自動的に有効な推論パスを探索して出力する。
    //
    //    Self-Consistency
    //    複数の推論パスを生成し、一番一貫性のある答えを多数決で選ぶ手法。
    //
    //    Tree-of-Thought (ToT)
    //    複数のアプローチ（分岐）を同時に検討し、各枝の評価から最適解を導く方法。
    //
    //    ReAct (Reasoning and Acting)
    //    単に推論するだけでなく、（必要に応じて）外部の行動やツール呼び出しも統合しながら問題解決する手法。たとえば、内部で「調べる」や「確認する」といった行動をシミュレートできます。
    //
    //    Reflective Reasoning（自己反省型推論）
    //    一度出力した回答に対して再評価・反省し、誤りや不足を検出して改善案を出す、いわば「自分の考えを振り返る」プロセスを組み込む手法。

    // 再帰プロンプト
    //【ステップ1】
    //初期回答を受け取ったら、以下の再帰プロセスに従って内容を改善してください。
    //
    //1. **1回目の再帰:**
    //　初期回答を「70点」と仮定します。
    //　→「各課題について、より具体的な事例や根本原因を加え、全体の具体性を高めた回答（80点相当）を生成してください。」
    //
    //2. **2回目の再帰:**
    //　1回目の回答を「80点」と仮定します。
    //　→「その内容をさらに踏み込んで、解決が非常に困難な側面やリスクも含め、より現実的な問題点を強調した回答（90点相当）を生成してください。」
    //
    //3. **3回目の再帰:**
    //　2回目の回答を「90点」と仮定します。
    //　→「最終的に、全体のバランスと論理の一貫性を再検証し、最終版として100点に相当する、完璧な回答を生成してください。」
    //    【ステップ1】
    //初期回答を受け取ったら、以下の再帰プロセスに従って内容を改善してください。
    //

    let meta_prompt = if task_description.contains("!!") {
        let summarized_sessions_str = search_similar_sessions(
            SearchType::First,
            &task_description,
            &multi_client,
            &anthropic_client,
        )
        .await?;
        format!(
            "You are an excellent software engineer.\n\
         Here are potentially similar tasks from the past:\n\
         {}\n\n\
         There are four available reasoning methods:\n\
         1. Chain-of-Thought (CoT): Generate answers by thinking step by step.\n\
         2. Automatic Chain-of-Thought (Auto-CoT): The model explores effective reasoning paths on its own.\n\
         3. Self-Consistency: Generate multiple reasoning paths and choose the most consistent answer by majority vote.\n\
         4. Tree-of-Thought (ToT): Consider multiple approaches in parallel and derive the optimal solution.\n\n\
         First, determine which reasoning method is most appropriate and briefly explain why.\n\
         If there are past task execution procedures that are similar to the current problem, please analyze them before starting the task.\n\
         Then, considering both your chosen reasoning method and past task execution procedures, address the problem comprehensively.",
            summarized_sessions_str
        )
    } else {
        format!(
            "You are an excellent software engineer.\n\
            There are four available reasoning methods:\n\
            1. Chain-of-Thought (CoT): Generate answers by thinking step by step.\n\
            2. Automatic Chain-of-Thought (Auto-CoT): The model explores effective reasoning paths on its own.\n\
            3. Self-Consistency: Generate multiple reasoning paths and choose the most consistent answer by majority vote.\n\
            4. Tree-of-Thought (ToT): Consider multiple approaches in parallel and derive the optimal solution.\n\n\
            First, determine which reasoning method is most appropriate and briefly explain why.\n\
            If there are past task execution procedures that are similar to the current problem, please analyze them before starting the task.\n\
            Then, considering both your chosen reasoning method and past task execution procedures, address the problem comprehensively.",
        )
    };

    let tool_notes = "Important notes regarding tool usage:\n\
        1. Avoid using the same tool consecutively.\n\
        2. Prioritize tools suggested by the user.\n\
        3. Before using each tool, evaluate whether it is truly necessary.";

    let system_message = format!("{}\n{}", meta_prompt, tool_notes);

    let initial_prompt = &format!(
        "{:?} Please consider and execute the next action.",
        task_description
    );

    let body = CreateMessageParams::new(RequiredMessageParams {
        model: "claude-3-5-haiku-20241022".to_string(),
        messages: vec![Message::new_text(Role::User, initial_prompt)],
        max_tokens: 1024,
    })
    .with_stream(true)
    .with_system(system_message)
    .with_tools(tools);

    process_message(&multi_client, anthropic_client, body).await?;
    clean_up(multi_client).await?;

    Ok(())
}

async fn process_message(
    multi_client: &MCPMultiClient,
    anthropic_client: AnthropicClient,
    mut body: CreateMessageParams,
) -> Result<(), McpError> {
    let mut state = TaskState {
        step: 0,
        description: "タスク開始".to_string(),
    };
    let mut error_count = 0;
    let mut comment_count = 0;

    let session_id = Uuid::new_v4().to_string();
    let start_time = Utc::now();
    let mut session_doc = SessionDocument {
        session_id: session_id.clone(),
        task_description: format!("{:?}", body.messages[0].content),
        messages: Vec::new(),
        metadata: SessionMetadata {
            total_steps: 0,
            used_tools: Vec::new(),
            completion_status: false,
            execution_time: 0,
            error_count: 0,
            comment_count: 0,
        },
        learning_metrics: None,
        updated_at: start_time,
    };

    session_doc.messages.push(MessageDocument {
        role: "user".to_string(),
        content: format!("{:?}", body.messages[0].content),
        timestamp: Utc::now(),
        tool_info: None,
        task_state: Some(state.clone()),
        feedback: None,
    });

    // 開始メッセージを表示
    println!(
        "{} {}",
        ">>>".bright_blue(),
        "[Session Started]".green().bold()
    );
    println!(
        "    セッションID: {}\n    タスク: {}\n",
        session_id.yellow(),
        format!("{:?}", body.messages[0].content).bright_white()
    );

    // ツールの実行と結果のフィードバック
    let mut user_selected_stop = false;
    loop {
        if user_selected_stop {
            return Ok(());
        }

        let response = match anthropic_client.create_message(Some(&body)).await {
            Ok(response) => {
                session_doc.messages.push(MessageDocument {
                    role: "assistant".to_string(),
                    content: format!("{:?}", response.content),
                    timestamp: Utc::now(),
                    tool_info: None,
                    task_state: None,
                    feedback: None,
                });
                body.messages.push(Message::new_text(
                    Role::Assistant,
                    format!("{:?}", response.content),
                ));
                response
            }
            Err(e) => {
                println!(
                    "{} {}: {}",
                    ">>>".red(),
                    "[Error]".red().bold(),
                    e.to_string()
                );
                error!("Error: {:?}", e);
                return Err(McpError::new(ErrorCode::InternalError, &e.to_string()));
            }
        };

        let content = match &response.content[0] {
            ContentBlock::Text { text } => text.to_string(),
            _ => String::new(),
        };

        // アシスタントの応答を色付きで表示
        println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());

        // テキスト内容を整形して表示
        let formatted_content = format_output_text(&content);
        println!("    {}", formatted_content);

        // ステップ情報を更新して表示
        state.step += 1;
        println!(
            "{} {}",
            "    [Step]".cyan(),
            state.step.to_string().bright_white()
        );
        println!(); // 空行を追加して見やすく

        // ツール使用の処理（ContentBlock::ToolUse を抽出）
        let mut tool_args: Vec<(String, HashMap<String, Value>)> = vec![];
        for c in response.content.iter() {
            match c {
                // まずは本来の ContentBlock::ToolUse 形式
                ContentBlock::ToolUse { name, input, .. } => {
                    if let Value::Object(map) = input {
                        let args: HashMap<String, Value> =
                            map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                        tool_args.push((name.clone(), args));
                    }
                }
                // Textブロック中に埋め込まれてしまった場合を拾う
                ContentBlock::Text { text } => {
                    debug!("Textブロック中に埋め込まれてしまった場合を拾う: {:?}", text);
                    let found = parse_embedded_tooluse_in_text(text);
                    debug!("埋め込まれてしまったツール使用: {:?}", found);
                    tool_args.extend(found);
                }
                _ => {}
            }
        }

        if tool_args.is_empty() {
            println!("{} {}", ">>>".yellow(), "[System]:".cyan().bold());
            println!("    {}", "次のアクションが指定されていません。".yellow());

            match get_next_action_with_options().await {
                ContinueActionImproved::Continue => {
                    println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
                    println!(
                        "    {}",
                        "ツールの指定がないため、会話を終了します。".bright_white()
                    );

                    // 完了ステータスを更新
                    session_doc.metadata.completion_status = true;
                    session_doc.metadata.execution_time =
                        (Utc::now() - start_time).num_seconds() as i64;
                    session_doc.metadata.total_steps = state.step;
                    session_doc.metadata.error_count = error_count;
                    session_doc.metadata.comment_count = comment_count;

                    // 完了メッセージ
                    println!(
                        "\n{} {}",
                        ">>>".green(),
                        "[Session Complete]".green().bold()
                    );
                    println!(
                        "    実行時間: {} 秒\n    ステップ数: {}\n    エラー数: {}\n    コメント数: {}",
                        session_doc.metadata.execution_time.to_string().yellow(),
                        state.step.to_string().yellow(),
                        error_count.to_string().yellow(),
                        comment_count.to_string().yellow()
                    );

                    return Ok(());
                }
                ContinueActionImproved::Stop => {
                    println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
                    println!(
                        "    {}",
                        "Got it! I'm stopping now as you requested.".bright_white()
                    );

                    // 完了メッセージ（ユーザー停止）
                    println!(
                        "\n{} {}",
                        ">>>".yellow(),
                        "[Session Stopped]".yellow().bold()
                    );
                    println!(
                        "    実行時間: {} 秒\n    ステップ数: {}",
                        (Utc::now() - start_time).num_seconds().to_string().yellow(),
                        state.step.to_string().yellow()
                    );

                    return Ok(());
                }
                ContinueActionImproved::Comment(comment) => {
                    println!("{} {}", ">>>".cyan(), "[Comment Added]:".cyan().bold());
                    println!("    {}", comment);

                    add_comment(&mut body, &mut session_doc, state.clone(), comment).await?;
                    comment_count += 1;
                    continue;
                }
                ContinueActionImproved::Feedback(feedback) => {
                    println!("{} {}", ">>>".magenta(), "[Feedback]:".magenta().bold());
                    println!("    {}", feedback);
                    println!("    RAGフィードバックを追加してタスクを終了します。");

                    add_rag_feedback(
                        session_doc,
                        true,
                        multi_client,
                        state,
                        error_count,
                        comment_count,
                        start_time,
                        feedback,
                    )
                    .await?;

                    println!(
                        "\n{} {}",
                        ">>>".magenta(),
                        "[Session Feedback Complete]".magenta().bold()
                    );

                    return Ok(());
                }
                ContinueActionImproved::Retry => {
                    println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
                    println!(
                        "    {}",
                        "リトライが選択されましたが、ツールがないため処理を終了します。".yellow()
                    );
                    return Ok(());
                }
            }
        }

        for (tool_name, args) in tool_args {
            // ツール実行確認メッセージを色付きで表示
            println!("{} {}", ">>>".yellow(), "[System]:".cyan().bold());
            println!(
                "    ツール実行の確認: {} {}",
                tool_name.bright_white().bold(),
                format_args_pretty(&args)
            );

            match get_next_action_with_options().await {
                ContinueActionImproved::Continue => {
                    // 実行開始メッセージ
                    println!(
                        "{} {}",
                        ">>>".bright_cyan(),
                        "[Tool Execution]:".bright_cyan().bold()
                    );
                    println!("    {} を実行中...", tool_name.bright_white());

                    let args_clone = args.clone();
                    match multi_client.call_tool(&tool_name, args).await {
                        Ok(result) => {
                            // 結果を色付きでフォーマットして表示
                            println!(
                                "{} {}",
                                ">>>".bright_green(),
                                "[Tool Result]:".bright_green().bold()
                            );

                            // ツール結果を整形して表示
                            let formatted_result = format_tool_result(&result);
                            println!("{}", formatted_result);
                            println!(); // 空行を追加して見やすく

                            // 結果を会話にフィードバック
                            body.messages.push(Message::new_text(
                                Role::User,
                                format!(
                                    "Called tool '{}' successfully. {:?}. state:{:?} Next action.",
                                    tool_name, result, state
                                ),
                            ));
                            session_doc.metadata.used_tools.push(tool_name.clone());
                            session_doc.messages.push(MessageDocument {
                                role: "system".to_string(),
                                content: format!("Tool result: {:?}", result),
                                timestamp: Utc::now(),
                                tool_info: Some(ToolInfo {
                                    name: tool_name.clone(),
                                    arguments: args_clone,
                                    result: Some(format!("{:?}", result)),
                                    success: true,
                                }),
                                task_state: Some(state.clone()),
                                feedback: None,
                            });
                        }
                        Err(e) => {
                            // エラーを目立つ色で表示
                            println!("{} {}", ">>>".red(), "[Tool Error]:".red().bold());
                            println!(
                                "    ツール '{}' の実行中にエラーが発生しました:",
                                tool_name.bright_white()
                            );
                            println!("    {}", e.to_string().red());
                            println!(); // 空行を追加

                            error!("Tool error occurred: {}", e);
                            body.messages.push(Message::new_text(
                                Role::User,
                                format!("Called tool '{}' failed: {}. Please retry or consider an alternative.", tool_name, e),
                            ));
                            error_count += 1;
                            session_doc.messages.push(MessageDocument {
                                role: "system".to_string(),
                                content: format!("Called tool '{}' failed: {}. Please retry or consider an alternative.", tool_name, e),
                                timestamp: Utc::now(),
                                tool_info: Some(ToolInfo {
                                    name: tool_name.clone(),
                                    arguments: args_clone,
                                    result: Some(format!("Called tool '{}' failed: {}", tool_name, e)),
                                    success: false,
                                }),
                                task_state: Some(state.clone()),
                                feedback: Some(FeedbackInfo {
                                    feedback_type: FeedbackType::Error.to_string(),
                                    content: e.to_string(),
                                }),
                            });
                            continue;
                        }
                    }
                }
                ContinueActionImproved::Comment(comment) => {
                    println!("{} {}", ">>>".cyan(), "[Comment Added]:".cyan().bold());
                    println!("    {}", comment);

                    add_comment(&mut body, &mut session_doc, state.clone(), comment).await?;
                    comment_count += 1;
                    continue;
                }
                ContinueActionImproved::Stop => {
                    user_selected_stop = true;
                    println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
                    println!(
                        "    {}",
                        "Got it! I'm stopping now as you requested.".bright_white()
                    );

                    // 完了メッセージ（ユーザー停止）
                    println!(
                        "\n{} {}",
                        ">>>".yellow(),
                        "[Session Stopped]".yellow().bold()
                    );
                    println!(
                        "    実行時間: {} 秒\n    ステップ数: {}",
                        (Utc::now() - start_time).num_seconds().to_string().yellow(),
                        state.step.to_string().yellow()
                    );
                    break;
                }
                ContinueActionImproved::Feedback(feedback) => {
                    println!("{} {}", ">>>".magenta(), "[Feedback]:".magenta().bold());
                    println!("    {}", feedback);
                    println!("    RAGフィードバックを追加してタスクを終了します。");

                    add_rag_feedback(
                        session_doc,
                        false,
                        multi_client,
                        state,
                        error_count,
                        comment_count,
                        start_time,
                        feedback,
                    )
                    .await?;

                    // 完了メッセージ（フィードバック付き）
                    println!(
                        "\n{} {}",
                        ">>>".magenta(),
                        "[Session Feedback Complete]".magenta().bold()
                    );

                    return Ok(());
                }
                ContinueActionImproved::Retry => {
                    // リトライ選択メッセージ
                    println!("{} {}", ">>>".blue(), "[Retry]:".blue().bold());
                    println!(
                        "    ツール '{}' の実行をリトライします。",
                        tool_name.bright_white()
                    );

                    // リトライ選択の場合は同じツール実行を再試行する
                    body.messages.push(Message::new_text(
                        Role::User,
                        format!("Retry to call tool '{}'.", tool_name),
                    ));
                    session_doc.messages.push(MessageDocument {
                        role: "user".to_string(),
                        content: format!("Retry to call tool '{}'.", tool_name),
                        timestamp: Utc::now(),
                        tool_info: None,
                        task_state: Some(state.clone()),
                        feedback: None,
                    });
                    continue;
                }
            }
        }
    }
}

// テキスト出力を整形する関数
fn format_output_text(text: &str) -> String {
    // 引用符を削除
    let text = if text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    };

    // JSONや構造化データの場合は整形
    if text.starts_with('{') && text.ends_with('}') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            return serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_string());
        }
    }

    // テキストを行ごとに整形
    text.lines()
        .map(|line| line.replace("\\n", "\n").replace("\\\"", "\""))
        .collect::<Vec<_>>()
        .join("\n")
}

// ツール結果を整形する関数
fn format_tool_result(result: &CallToolResult) -> String {
    use colored::*;

    let mut formatted = String::new();

    for content in &result.content {
        match content {
            Text(TextContent { text, .. }) => {
                // テキスト結果を行ごとに整形
                let indented = text
                    .lines()
                    .map(|line| {
                        // JSONっぽい行の場合は整形を試みる
                        if (line.contains("{") && line.contains("}"))
                            || (line.contains("[") && line.contains("]"))
                        {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                                return format!(
                                    "    {}",
                                    serde_json::to_string_pretty(&json)
                                        .unwrap_or_else(|_| line.to_string())
                                        .bright_white()
                                );
                            }
                        }
                        format!("    {}", line.bright_white())
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                formatted.push_str(&indented);
            }
            // 他の型の結果に対する処理も追加可能
            _ => formatted.push_str(&format!("    {:?}", content)),
        }
    }

    formatted
}

// 引数をきれいに表示する関数
fn format_args_pretty(args: &HashMap<String, Value>) -> String {
    use colored::*;

    let mut parts = Vec::new();
    for (key, value) in args {
        let formatted_value = match value {
            Value::String(s) => format!("\"{}\"", s).green(),
            Value::Number(n) => n.to_string().yellow(),
            Value::Bool(b) => {
                if *b {
                    "true".bright_green()
                } else {
                    "false".red()
                }
            }
            Value::Null => "null".bright_black(),
            Value::Array(_) | Value::Object(_) => serde_json::to_string_pretty(value)
                .unwrap_or_else(|_| format!("{:?}", value))
                .bright_white(),
        };
        parts.push(format!("{}={}", key.cyan(), formatted_value));
    }

    format!("({})", parts.join(", "))
}

/// clean up the multi client
///
/// # Arguments
///
/// * `multi_client` - the multi client
///
/// # Returns
///
/// * `Result<(), McpError>` - the result
///
/// # Errors
///
/// * `McpError` - the error
async fn clean_up(multi_client: MCPMultiClient) -> Result<(), McpError> {
    // println!(">>> [Assistant]: Cleaning up... Waiting for Ctrl+C.");
    // let (tx, rx) = tokio::sync::oneshot::channel();
    // tokio::spawn(async move {
    //     let _ = tokio::signal::ctrl_c().await;
    //     let _ = tx.send(());
    // });
    // let _ = rx.await;

    multi_client.close().await?;
    println!("{} {}", ">>>".yellow(), "[System]:".cyan().bold());
    println!(
        "    {}",
        "Thank you for using the assistant. Goodbye!".yellow()
    );

    Ok(())
}

/// MCPMultiClient implementation
///
/// # Arguments
///
/// * `config` - the config
///
/// # Returns
///
/// * `Result<(), McpError>` - the result
///
/// # Errors
///
/// * `McpError` - the error
impl MCPMultiClient {
    /// new
    ///
    /// # Arguments
    ///
    /// * `config` - the config
    ///
    /// # Returns
    ///
    /// * `Self` - the multi client
    pub async fn new(config: MCPServersConfig) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            tool_map: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// initialize
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Tool>, McpError>` - the result
    ///
    /// # Errors
    ///
    /// * `McpError` - the error
    pub async fn initialize(&self) -> Result<Vec<Tool>, McpError> {
        let mut all_tools = Vec::new();
        for (client_type, server_config) in &self.config.mcp_servers {
            debug!("MCPサーバー '{}' を初期化中...", client_type);
            let mut client = Arc::new(
                Client::new(
                    Implementation {
                        name: client_type.clone(),
                        version: "0.1.0".to_string(),
                    },
                    ClientOptions {
                        capabilities: ClientCapabilities {
                            sampling: None,
                            experimental: None,
                            roots: None,
                        },
                        enforce_strict_capabilities: Some(false),
                    },
                )
                .await,
            );
            let transport = StdioClientTransport::new(StdioServerParameters {
                command: server_config.command.clone(),
                args: server_config.args.clone(),
                env: server_config.env.clone(),
                stderr: Some(StdioMode::Inherit),
            });
            if let Some(client_ref) = Arc::get_mut(&mut client) {
                debug!("{} に接続します...", client_type);
                client_ref.connect(Box::new(transport)).await?;
                let tools = client_ref.list_tools(None).await?.tools;
                let mut tool_map = self.tool_map.write().await;
                for tool in &tools {
                    tool_map.insert(tool.name.clone(), client_type.clone());
                }
                all_tools.extend(tools);
                self.clients
                    .write()
                    .await
                    .insert(client_type.clone(), client);
            }
        }
        Ok(all_tools.into_anthropic_tools())
    }

    /// get the client for the tool
    ///
    /// # Arguments
    ///
    /// * `tool_name` - the tool name
    ///
    /// # Returns
    ///
    /// * `Option<Arc<Client>>` - the client
    ///
    /// # Errors
    ///
    /// * `McpError` - the error
    pub async fn get_client_for_tool(&self, tool_name: &str) -> Option<Arc<Client>> {
        let tool_map = self.tool_map.read().await;
        let client_type = tool_map.get(tool_name)?;
        let clients = self.clients.read().await;
        clients.get(client_type).cloned()
    }

    /// call the tool
    ///
    /// # Arguments
    ///
    /// * `tool_name` - the tool name
    /// * `arguments` - the arguments
    ///
    /// # Returns
    ///
    /// * `Result<CallToolResult, McpError>` - the result
    ///
    /// # Errors
    ///
    /// * `McpError` - the error
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: HashMap<String, Value>,
    ) -> Result<CallToolResult, McpError> {
        info!(">>> [Assistant]: ツール '{}' を呼び出し中...", tool_name);
        let client = self.get_client_for_tool(tool_name).await.ok_or_else(|| {
            McpError::new(
                ErrorCode::MethodNotFound,
                &format!("ツールが見つかりません: {}", tool_name),
            )
        })?;
        client
            .call_tool(CallToolRequest {
                name: tool_name.to_string(),
                arguments: Some(arguments),
            })
            .await
    }

    /// close the multi client
    ///
    /// # Returns
    ///
    /// * `Result<(), McpError>` - the result
    ///
    /// # Errors
    ///
    /// * `McpError` - the error
    pub async fn close(&self) -> Result<(), McpError> {
        let mut clients = self.clients.write().await;
        for (_, client) in clients.drain() {
            match Arc::try_unwrap(client) {
                Ok(client) => {
                    client.close().await?;
                }
                Err(_) => continue,
            }
        }
        Ok(())
    }
}
