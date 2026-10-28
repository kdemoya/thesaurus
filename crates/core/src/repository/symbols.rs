use super::*;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
struct Lsp {
    child: tokio::process::Child,
    input: tokio::process::ChildStdin,
    output: BufReader<tokio::process::ChildStdout>,
    next: u64,
}
impl Lsp {
    async fn send(&mut self, value: Value) -> Result<()> {
        let bytes = serde_json::to_vec(&value)?;
        self.input
            .write_all(format!("Content-Length: {}\r\n\r\n", bytes.len()).as_bytes())
            .await?;
        self.input.write_all(&bytes).await?;
        self.input.flush().await?;
        Ok(())
    }
    async fn read(&mut self) -> Result<Value> {
        let mut header = Vec::new();
        loop {
            if header.len() >= 8192 {
                bail!("invalid LSP header size");
            }
            header.push(
                self.output
                    .read_u8()
                    .await
                    .context("invalid: symbol service closed its connection")?,
            );
            if header.ends_with(b"\r\n\r\n") || header.ends_with(b"\n\n") {
                break;
            }
        }
        let mut length = None;
        for line in std::str::from_utf8(&header)?.lines() {
            if let Some((key, value)) = line.split_once(':') {
                if key.eq_ignore_ascii_case("Content-Length") {
                    if length.is_some() {
                        bail!("invalid: duplicate LSP content length");
                    }
                    length = Some(value.trim().parse::<usize>()?);
                }
            }
        }
        let len = length.context("invalid: missing LSP content length")?;
        if len > 2_000_000 {
            bail!("invalid: symbol service response exceeds 2MB");
        }
        let mut bytes = vec![0; len];
        self.output.read_exact(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }
    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next;
        self.next += 1;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        for _ in 0..500 {
            let message = self.read().await?;
            if message["id"] == id && message.get("method").is_none() {
                if message.get("error").is_some() {
                    bail!(
                        "invalid: symbol service rejected {method}: {}",
                        message["error"]
                    );
                }
                return Ok(message["result"].clone());
            }
            if message.get("id").is_some() && message.get("method").is_some() {
                let method = message["method"].as_str().unwrap_or_default();
                let response = if method == "window/workDoneProgress/create" {
                    json!({"jsonrpc":"2.0","id":message["id"],"result":null})
                } else if method == "workspace/configuration" {
                    let n = message["params"]["items"]
                        .as_array()
                        .map(Vec::len)
                        .unwrap_or(0)
                        .min(30);
                    json!({"jsonrpc":"2.0","id":message["id"],"result":vec![Value::Null;n]})
                } else {
                    json!({"jsonrpc":"2.0","id":message["id"],"error":{"code":-32601,"message":"Thesaurus permits read-only symbol requests only"}})
                };
                self.send(response).await?;
            }
        }
        bail!("invalid: symbol service exceeded message limit")
    }
}
async fn query(
    repo: &RepositoryConnection,
    service: &SymbolService,
    req: &SymbolQueryRequest,
    text: &str,
    path: &Path,
) -> Result<Vec<Value>> {
    let mut child = Command::new(&service.command)
        .args(&service.args)
        .current_dir(&repo.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("invalid: cannot start approved symbol service")?;
    let input = child.stdin.take().context("LSP input missing")?;
    let output = BufReader::new(child.stdout.take().context("LSP output missing")?);
    let mut client = Lsp {
        child,
        input,
        output,
        next: 1,
    };
    let root = reqwest::Url::from_directory_path(&repo.path)
        .map_err(|_| anyhow::anyhow!("invalid repository URI"))?
        .to_string();
    let uri = reqwest::Url::from_file_path(path)
        .map_err(|_| anyhow::anyhow!("invalid file URI"))?
        .to_string();
    let init=client.request("initialize",json!({"processId":null,"rootUri":root,"workspaceFolders":[{"uri":root,"name":repo.name}],"clientInfo":{"name":"Thesaurus","version":"0.1"},"capabilities":{"general":{"positionEncodings":["utf-16"]},"workspace":{"configuration":true,"applyEdit":false},"textDocument":{"definition":{"linkSupport":true},"references":{}}},"initializationOptions":{"cargo":{"buildScripts":{"enable":false}},"procMacro":{"enable":false}}})).await?;
    if init["capabilities"]["positionEncoding"]
        .as_str()
        .is_some_and(|s| s != "utf-16")
    {
        bail!("invalid: symbol service must support UTF-16 positions");
    }
    let capability = if req.method == "definition" {
        "definitionProvider"
    } else {
        "referencesProvider"
    };
    if init["capabilities"][capability].is_null() || init["capabilities"][capability] == false {
        bail!("invalid: symbol service does not advertise requested capability");
    }
    client
        .send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}))
        .await?;
    client.send(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":service.language_id,"version":1,"text":text}}})).await?;
    let mut params = json!({"textDocument":{"uri":uri},"position":{"line":req.line,"character":req.utf16_character}});
    if req.method == "references" {
        params["context"] = json!({"includeDeclaration":true});
    }
    let result = client
        .request(&format!("textDocument/{}", req.method), params)
        .await?;
    let _ = client.request("shutdown", Value::Null).await;
    let _ = client.send(json!({"jsonrpc":"2.0","method":"exit"})).await;
    let _ = client.child.kill().await;
    Ok(match result {
        Value::Null => vec![],
        Value::Array(a) => a,
        Value::Object(_) => vec![result],
        _ => bail!("invalid symbol result shape"),
    })
}
impl Store {
    pub async fn repository_symbols(
        &self,
        id: &str,
        req: SymbolQueryRequest,
    ) -> Result<SymbolQueryResult> {
        if !req.approve_service {
            bail!(
                "forbidden: explicitly approve this symbol service request; it can index the selected repository"
            );
        }
        if !["definition", "references"].contains(&req.method.as_str()) {
            bail!("invalid: choose definition or references");
        }
        let repo = self.repository(id, &Access::admin()).await?;
        if req.expected_version != repo.version {
            bail!("conflict: repository service settings changed");
        }
        let service = repo
            .symbol_service
            .clone()
            .context("invalid: approve/configure a symbol service first")?;
        let mut tx = self.pool.begin().await?;
        let (digest, old) = operation(
            &mut tx,
            "repository-symbols",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(old) = old {
            let json: String =
                sqlx::query_scalar("SELECT record FROM repo_symbol_queries WHERE id=?")
                    .bind(old)
                    .fetch_one(&mut *tx)
                    .await?;
            tx.commit().await?;
            return Ok(serde_json::from_str(&json)?);
        }
        let scope = project_sources(&mut tx, &repo.project_id).await?;
        tx.commit().await?;
        let path = safe_file(&repo, &req.file).await?;
        if tokio::fs::metadata(&path).await?.len() > 100000 {
            bail!("invalid: selected symbol file exceeds 100KB");
        }
        let text = read_text(&path)
            .await
            .context("invalid: symbol file must be UTF-8")?;
        let line = text
            .lines()
            .nth(req.line as usize)
            .context("invalid: line is outside file (LSP uses zero-based lines)")?;
        if req.utf16_character as usize > line.encode_utf16().count() {
            bail!("invalid: UTF-16 position is outside line");
        }
        let pinned = commit(&repo.path, "HEAD").await?;
        let raw = tokio::time::timeout(
            std::time::Duration::from_secs(45),
            query(&repo, &service, &req, &text, &path),
        )
        .await
        .context("invalid: symbol service timed out after 45 seconds")??;
        let root = tokio::fs::canonicalize(&repo.path).await?;
        let mut locations = vec![];
        let mut omitted = 0usize;
        for value in raw {
            if locations.len() >= 100 {
                omitted += 1;
                continue;
            }
            let uri = value["uri"]
                .as_str()
                .or_else(|| value["targetUri"].as_str());
            let Some(uri) = uri else {
                omitted += 1;
                continue;
            };
            let Some(file) = reqwest::Url::parse(uri)
                .ok()
                .and_then(|u| u.to_file_path().ok())
            else {
                omitted += 1;
                continue;
            };
            let Ok(file) = tokio::fs::canonicalize(file).await else {
                omitted += 1;
                continue;
            };
            let Ok(relative) = file.strip_prefix(&root) else {
                omitted += 1;
                continue;
            };
            let Some(relative) = relative.to_str() else {
                omitted += 1;
                continue;
            };
            if !allowed(&repo, relative) {
                omitted += 1;
                continue;
            }
            let Ok(metadata) = tokio::fs::metadata(&file).await else {
                omitted += 1;
                continue;
            };
            if !metadata.is_file() || metadata.len() > 100000 {
                omitted += 1;
                continue;
            }
            let Ok(body) = read_text(&file).await else {
                omitted += 1;
                continue;
            };
            let range = if value.get("range").is_some() {
                &value["range"]
            } else {
                value
                    .get("targetSelectionRange")
                    .unwrap_or(&value["targetRange"])
            };
            let Some(start_line) = range["start"]["line"]
                .as_u64()
                .filter(|n| *n <= u32::MAX as u64)
            else {
                omitted += 1;
                continue;
            };
            let Some(start_char) = range["start"]["character"]
                .as_u64()
                .filter(|n| *n <= u32::MAX as u64)
            else {
                omitted += 1;
                continue;
            };
            let end_line = range["end"]["line"].as_u64().unwrap_or(start_line);
            let end_char = range["end"]["character"].as_u64().unwrap_or(start_char);
            if end_line > u32::MAX as u64 || end_char > u32::MAX as u64 || end_line < start_line {
                omitted += 1;
                continue;
            }
            let start = body.lines().nth(start_line as usize);
            let end = body.lines().nth(end_line as usize);
            if start.is_none_or(|l| start_char as usize > l.encode_utf16().count())
                || end.is_none_or(|l| end_char as usize > l.encode_utf16().count())
                || (end_line == start_line && end_char < start_char)
            {
                omitted += 1;
                continue;
            }
            let excerpt = body
                .lines()
                .skip(start_line as usize)
                .take(3)
                .collect::<Vec<_>>()
                .join("\n")
                .chars()
                .take(2000)
                .collect();
            locations.push(SymbolLocation {
                file: relative.into(),
                line: start_line as u32,
                utf16_character: start_char as u32,
                end_line: end_line as u32,
                end_utf16_character: end_char as u32,
                excerpt,
            });
        }
        if commit(&repo.path, "HEAD").await? != pinned || read_text(&path).await? != text {
            bail!("conflict: repository HEAD or selected file changed during symbol lookup");
        }
        let result=SymbolQueryResult{id:uuid::Uuid::new_v4().to_string(),repository_id:id.into(),project_id:repo.project_id.clone(),source_ids:scope,commit:pinned,file:req.file.clone(),file_hash:hash(&text),line:req.line,utf16_character:req.utf16_character,method:req.method.clone(),locations,omitted_locations:omitted,service,created_at:now(),notes:vec!["Positions are zero-based UTF-16 LSP positions. Returned references are filtered to approved file prefixes. The approved service may index other files within its repository process; Thesaurus reads only the selected file and up to 100 returned locations (100KB/file). No source documents are imported, no commands/edits from the server are accepted.".into()]};
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (_, old) = operation(
            &mut tx,
            "repository-symbols",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(old) = old {
            let record: String =
                sqlx::query_scalar("SELECT record FROM repo_symbol_queries WHERE id=?")
                    .bind(old)
                    .fetch_one(&mut *tx)
                    .await?;
            tx.commit().await?;
            return Ok(serde_json::from_str(&record)?);
        }
        let current = load(&mut tx, id, &Access::admin()).await?;
        if current.version != req.expected_version
            || project_sources(&mut tx, &repo.project_id).await? != result.source_ids
        {
            bail!("conflict: repository/project configuration changed");
        }
        sqlx::query(
            "INSERT INTO repo_symbol_queries(id,repository_id,record,created_at) VALUES(?,?,?,?)",
        )
        .bind(&result.id)
        .bind(id)
        .bind(serde_json::to_string(&result)?)
        .bind(&result.created_at)
        .execute(&mut *tx)
        .await?;
        remember(&mut tx, &req.idempotency_key, &digest, &result.id).await?;
        event(&mut tx, "repository", id).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn repository_symbol_history(
        &self,
        id: &str,
        access: &Access,
    ) -> Result<Vec<SymbolQueryResult>> {
        self.repository(id, access).await?;
        let rows=sqlx::query_scalar::<_,String>("SELECT record FROM repo_symbol_queries WHERE repository_id=? ORDER BY created_at DESC LIMIT 30").bind(id).fetch_all(&self.pool).await?;
        let mut tx = self.pool.begin().await?;
        let mut results = vec![];
        for r in rows {
            let mut r: SymbolQueryResult = serde_json::from_str(&r)?;
            if permit(&mut tx, &r.source_ids, access).await.is_ok() {
                if access.source_ids.is_some() {
                    r.service.command = "[approved local service]".into();
                    r.service.args.clear();
                }
                results.push(r);
            }
        }
        tx.commit().await?;
        Ok(results)
    }
}
