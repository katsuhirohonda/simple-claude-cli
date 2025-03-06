use anthropic_ai_sdk::clients::AnthropicClient;
use colored::*;

use anthropic_ai_sdk::types::message::{
    CreateMessageParams, Message, MessageClient, MessageError, RequiredMessageParams, Role, Tool,
};
use tracing::error;

///
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

    let meta_prompt = format!(
        "You are an excellent software engineer.\n\
            There are four available reasoning methods:\n\
            1. Chain-of-Thought (CoT): Generate answers by thinking step by step.\n\
            2. Automatic Chain-of-Thought (Auto-CoT): The model explores effective reasoning paths on its own.\n\
            3. Self-Consistency: Generate multiple reasoning paths and choose the most consistent answer by majority vote.\n\
            4. Tree-of-Thought (ToT): Consider multiple approaches in parallel and derive the optimal solution.\n\n\
            First, determine which reasoning method is most appropriate and briefly explain why.\n\
            If there are past task execution procedures that are similar to the current problem, please analyze them before starting the task.\n\
            Then, considering both your chosen reasoning method and past task execution procedures, address the problem comprehensively.",
    );

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
    .with_system(system_message);

    let response = match anthropic_client.create_message(Some(&body)).await {
        Ok(response) => {
            //body.messages.push(Message::new_text(
            //    Role::Assistant,
            //    format!("{:?}", response.content),
            //));
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
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ));
        }
    };

    Ok(())
}
