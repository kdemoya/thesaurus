use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum DeliveryCommands {
    Context {
        query: String,
        #[arg(long, default_value_t = 12000)]
        characters: usize,
        #[arg(long, default_value_t = 4000)]
        tokens: usize,
        #[arg(long)]
        source: Vec<String>,
    },
    Receipt {
        thread: String,
        context: String,
    },
    Update {
        thread: String,
        context: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["acknowledged","completion_claimed","completed","reopened"])]
        action: String,
        #[arg(long)]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// Attach to an existing run. This never starts/resumes a model request.
    Observe {
        run: String,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t = 1)]
        polls: u32,
    },
}
fn id(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 100
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        anyhow::bail!("invalid id");
    }
    Ok(())
}
pub async fn execute(client: &Client, cmd: DeliveryCommands) -> Result<Value> {
    use reqwest::Method;
    match cmd{
    DeliveryCommands::Context{query,characters,tokens,source}=>client.call(Method::POST,"/api/v1/context/budgeted",Some(json!({"query":query,"character_budget":characters,"token_budget":tokens,"source_ids":if source.is_empty(){None}else{Some(source)}}))).await,
    DeliveryCommands::Receipt{thread,context}=>{id(&thread)?;id(&context)?;client.call(Method::GET,&format!("/api/v1/threads/{thread}/contexts/{context}/receipt"),None).await},
    DeliveryCommands::Update{thread,context,expected_version,action,note,idempotency_key}=>{id(&thread)?;id(&context)?;client.call(Method::POST,&format!("/api/v1/threads/{thread}/contexts/{context}/receipt"),Some(json!({"expected_version":expected_version,"action":action,"note":note,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}))).await},
    DeliveryCommands::Observe{run,mut cursor,polls}=>{id(&run)?;if polls==0 || polls>300{anyhow::bail!("invalid: choose 1–300 observation polls");}if cursor.as_ref().is_some_and(|c|!c.bytes().all(|b|b.is_ascii_hexdigit()) || c.len()>100){anyhow::bail!("invalid cursor");}let mut result=json!({});for n in 0..polls{let path=format!("/api/v1/runtime/runs/{run}/observe{}",cursor.as_ref().map(|c|format!("?cursor={c}")).unwrap_or_default());result=client.call(Method::GET,&path,None).await?;if result["changed"]==true || n+1==polls{println!("{}",serde_json::to_string(&result)?);}cursor=result["cursor"].as_str().map(str::to_owned);if !matches!(result["run"]["status"].as_str(),Some("starting"|"running"|"waiting_permission"|"cancelling")){break;}if n+1<polls{tokio::time::sleep(std::time::Duration::from_secs(2)).await;}}Ok(json!({"run_id":run,"cursor":cursor,"status":result["run"]["status"],"observation_only":true}))}
}
}
