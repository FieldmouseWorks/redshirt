//! Optional, bounded coding-model diagnostics through a trusted local CLI.
use super::*;
use crate::{CancellationToken, evidence::decode};
use async_trait::async_trait;
use std::{fs, path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

// Even worst-case JSON escaping fits the enclosing 256 KiB call record.
pub const MAX_STDOUT: usize = 32768;
pub const MAX_STDERR: usize = 4096;
pub const DEADLINE_SECS: u64 = 60;
pub const INSTRUCTIONS: &str = "Analyze only the task and source evidence in the supplied JSON. Preserve its mandatory project policy. Return exactly the requested JSON choice object. Use insufficient if the evidence does not establish an answer. Do not call tools, ask questions, access files, or use outside knowledge to fill missing implementation details. No action or code execution is authorized.";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub model: String,
    pub effort: String,
    pub cli_version: String,
    pub cli_sha256: String,
    pub catalog: Value,
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        let models = self.catalog["models"]
            .as_array()
            .ok_or_else(|| Stop::from("codex_catalog"))?;
        require(
            !self.model.is_empty()
                && self.model.len() <= 80
                && self
                    .model
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-._".contains(&c))
                && self.effort == "low"
                && self.cli_version == "codex-cli 0.154.0"
                && self.cli_sha256.len() == 64
                && self.cli_sha256.bytes().all(|c| c.is_ascii_hexdigit())
                && models.len() == 1
                && models[0]["slug"] == self.model
                && models[0]["tool_mode"].is_null()
                && models[0]["apply_patch_tool_type"].is_null()
                && models[0]["experimental_supported_tools"] == json!([])
                && models[0]["node_repl_disabled"] == true
                && encoded(self)?.len() <= 32768,
            "codex_profile",
        )
    }
    pub fn digest(&self) -> Result<String> {
        Ok(sha256(&encoded(self)?))
    }
}

pub fn request(
    profile: &Profile,
    state: Value,
    choices: &BTreeMap<String, String>,
) -> Result<Value> {
    Ok(
        json!({"model":profile.model,"effort":profile.effort,"profile_sha256":profile.digest()?,
        "instructions":DIAGNOSIS,"state":state,"choices":choices}),
    )
}

/// A CLI transcript, not an HTTP receipt or a claim of a server-side snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transcript {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub stop: Option<String>,
}

pub fn parse(transcript: &Transcript, request: &Value) -> Result<(String, Value)> {
    require(
        transcript.stop.is_none() && transcript.exit_code == Some(0),
        "codex_failed",
    )?;
    require(
        transcript.stdout.len() <= MAX_STDOUT && transcript.stderr.len() <= MAX_STDERR,
        "codex_output_size",
    )?;
    let (mut threads, mut turns, mut completed) = (0, 0, 0);
    let (mut answer, mut usage) = (None, None);
    for line in transcript.stdout.lines() {
        let event = decode(line.as_bytes())?;
        match event["type"].as_str() {
            Some("thread.started") if turns == 0 => threads += 1,
            Some("turn.started") if threads == 1 && completed == 0 => turns += 1,
            Some("item.started" | "item.updated")
                if turns == 1
                    && completed == 0
                    && answer.is_none()
                    && event["item"]["type"] == "reasoning" => {}
            Some("item.completed") if turns == 1 && completed == 0 => {
                match event["item"]["type"].as_str() {
                    Some("agent_message") if answer.is_none() => {
                        let text = event["item"]["text"]
                            .as_str()
                            .ok_or_else(|| Stop::from("codex_answer"))?;
                        let response = decode(text.as_bytes())?;
                        require(
                            response.as_object().is_some_and(|o| o.len() == 1),
                            "codex_answer",
                        )?;
                        let choice = response["choice"]
                            .as_str()
                            .ok_or_else(|| Stop::from("codex_answer"))?;
                        require(
                            request["choices"]
                                .as_object()
                                .is_some_and(|c| c.contains_key(choice)),
                            "codex_choice",
                        )?;
                        answer = Some(choice.to_owned());
                    }
                    Some("reasoning") if answer.is_none() => {}
                    _ => return Err("codex_unexpected_event".into()),
                }
            }
            Some("turn.completed") if turns == 1 && answer.is_some() && completed == 0 => {
                let u = &event["usage"];
                for field in ["input_tokens", "cached_input_tokens", "output_tokens"] {
                    require(
                        u[field].as_u64().is_some_and(|n| n <= 1_000_000),
                        "codex_usage",
                    )?;
                }
                require(
                    u["cached_input_tokens"].as_u64() <= u["input_tokens"].as_u64(),
                    "codex_usage",
                )?;
                usage = Some(u.clone());
                completed += 1;
            }
            _ => return Err("codex_unexpected_event".into()),
        }
    }
    require(
        threads == 1 && turns == 1 && completed == 1,
        "codex_incomplete",
    )?;
    Ok((
        answer.ok_or_else(|| Stop::from("codex_answer"))?,
        usage.ok_or_else(|| Stop::from("codex_usage"))?,
    ))
}

#[async_trait]
pub trait Transport: Send {
    fn is_live(&self) -> bool {
        false
    }
    async fn run(&mut self, request: &Value, cancel: &CancellationToken) -> Result<Transcript>;
}

pub struct Diagnostic<T> {
    transport: T,
    profile: Profile,
    calls: u32,
    receipt: Option<Value>,
}
impl<T: Transport> Diagnostic<T> {
    pub fn new(transport: T, profile: Profile) -> Result<Self> {
        profile.validate()?;
        Ok(Self {
            transport,
            profile,
            calls: 0,
            receipt: None,
        })
    }
    pub fn calls(&self) -> u32 {
        self.calls
    }
    pub fn profile(&self) -> &Profile {
        &self.profile
    }
    pub fn is_live(&self) -> bool {
        self.transport.is_live()
    }
    pub fn take_evidence(&mut self) -> Option<Value> {
        self.receipt.take()
    }
    pub async fn ask(&mut self, body: Value, cancel: &CancellationToken) -> Result<()> {
        require(
            self.receipt.is_none() && self.calls < 8,
            "codex_budget_or_receipt_pending",
        )?;
        require(
            body["model"] == self.profile.model
                && body["profile_sha256"] == self.profile.digest()?
                && encoded(&body)?.len() <= 32768,
            "codex_request",
        )?;
        self.calls += 1;
        self.receipt = Some(json!({"version":1,"kind":"codex_exec","call":self.calls,
            "model":self.profile.model,"request":body,"request_sha256":sha256(&encoded(&body)?),
            "reserved_turns":1,"deadline_secs":DEADLINE_SECS,"reserved_usd":null,
            "billed_usd":null,"outcome":"interrupted"}));
        let result = self.transport.run(&body, cancel).await;
        let receipt = self.receipt.as_mut().expect("active receipt");
        let outcome = match result {
            Ok(transcript) => {
                receipt["transcript"] = json!(transcript);
                if let Some(error) = &transcript.stop {
                    Err(Stop(error.clone()))
                } else {
                    parse(&transcript, &body).map(|(_, usage)| {
                        receipt["usage"] = usage;
                    })
                }
            }
            Err(error) => Err(error),
        };
        if let Err(error) = &outcome {
            receipt["outcome"] = "failed".into();
            receipt["error"] = error.0.clone().into();
        } else {
            receipt["outcome"] = "accepted".into();
        }
        outcome
    }
}

pub struct Cli {
    executable: PathBuf,
    profile: Profile,
}

pub fn binary_digest(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|_| Stop::from("codex_executable"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| Stop::from("codex_executable"))?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

impl Cli {
    pub fn new(executable: PathBuf, profile: Profile) -> Result<Self> {
        profile.validate()?;
        require(
            executable.is_absolute() && binary_digest(&executable)? == profile.cli_sha256,
            "codex_executable_identity",
        )?;
        Ok(Self {
            executable,
            profile,
        })
    }
}

// No model-visible shell, file, browser, connector, plugin, or delegation tool.
// The CLI may advertise its unavailable-in-exec user-input tool; any invocation
// or extra message invalidates the transcript. No trusted command is model-authored.
const DISABLED: &[&str] = &[
    "apps",
    "plugins",
    "remote_plugin",
    "workspace_dependencies",
    "shell_tool",
    "shell_snapshot",
    "unified_exec",
    "multi_agent",
    "multi_agent_v2",
    "memories",
    "hooks",
    "browser_use",
    "browser_use_external",
    "browser_use_full_cdp_access",
    "computer_use",
    "image_generation",
    "view_image",
    "in_app_browser",
    "in_app_chat",
    "in_app_local_automation",
    "code_mode",
    "code_mode_host",
    "code_mode_only",
    "goals",
    "skill_search",
    "skill_mcp_dependency_install",
    "tool_suggest",
    "sleep_tool",
    "unbounded_connection_retries",
    "enable_request_compression",
    "personality",
];

#[async_trait]
impl Transport for Cli {
    fn is_live(&self) -> bool {
        true
    }
    async fn run(&mut self, request: &Value, cancel: &CancellationToken) -> Result<Transcript> {
        require(!cancel.is_cancelled(), "cancelled")?;
        let temporary = tempfile::tempdir().map_err(|_| Stop::from("codex_temporary_directory"))?;
        let root = temporary.path();
        let schema = json!({"type":"object","properties":{"choice":{"type":"string",
            "enum":request["choices"].as_object().ok_or_else(|| Stop::from("codex_choices"))?.keys().collect::<Vec<_>>()}},
            "required":["choice"],"additionalProperties":false});
        for (file, bytes) in [
            ("schema.json", encoded(&schema)?),
            ("catalog.json", encoded(&self.profile.catalog)?),
            ("instructions.md", INSTRUCTIONS.as_bytes().to_vec()),
        ] {
            fs::write(root.join(file), bytes)
                .map_err(|_| Stop::from("codex_temporary_directory"))?;
        }
        let mut command = Command::new(&self.executable);
        command.env_clear();
        for key in [
            "PATH",
            "HOME",
            "USER",
            "LOGNAME",
            "SHELL",
            "LANG",
            "LC_ALL",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_RUNTIME_DIR",
            "CODEX_HOME",
            "TMPDIR",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command
            .args([
                "exec",
                "--ignore-user-config",
                "--ignore-rules",
                "--ephemeral",
                "--skip-git-repo-check",
                "--json",
                "--color",
                "never",
                "--sandbox",
                "read-only",
                "-m",
                &self.profile.model,
                "-C",
            ])
            .arg(root)
            .arg("--output-schema")
            .arg(root.join("schema.json"));
        for setting in [
            "model_provider=\"redshirt_diagnostic\"".to_owned(),
            "model_providers.redshirt_diagnostic={name=\"Redshirt diagnostic\",requires_openai_auth=true,wire_api=\"responses\",request_max_retries=0,stream_max_retries=0,supports_websockets=false}".into(),
            format!("model_reasoning_effort={:?}",self.profile.effort),
            format!("model_catalog_json={:?}",root.join("catalog.json")),
            format!("model_instructions_file={:?}",root.join("instructions.md")),
            "project_doc_max_bytes=0".into(),"web_search=\"disabled\"".into(),
            "memories.generate_memories=false".into(),"memories.use_memories=false".into(),
            "agents.enabled=false".into(),"features.skip_host_skill_discovery=true".into(),
            "suppress_unstable_features_warning=true".into(),
        ] { command.arg("-c").arg(setting); }
        for feature in DISABLED {
            command.arg("-c").arg(format!("features.{feature}=false"));
        }
        command
            .arg("-")
            .current_dir(root)
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|_| Stop::from("codex_spawn"))?;
        let mut input = child.stdin.take().ok_or_else(|| Stop::from("codex_pipe"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| Stop::from("codex_pipe"))?
            .take(MAX_STDOUT as u64 + 1);
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| Stop::from("codex_pipe"))?
            .take(MAX_STDERR as u64 + 1);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let payload = encoded(request)?;
        let status = {
            let work = async {
                let write = async move {
                    input.write_all(&payload).await?;
                    input.shutdown().await?;
                    drop(input);
                    Ok::<(), std::io::Error>(())
                };
                let read_out = async {
                    stdout.read_to_end(&mut out).await?;
                    if out.len() > MAX_STDOUT {
                        return Err(std::io::Error::other("output cap"));
                    }
                    Ok(())
                };
                let read_err = async {
                    stderr.read_to_end(&mut err).await?;
                    if err.len() > MAX_STDERR {
                        return Err(std::io::Error::other("output cap"));
                    }
                    Ok(())
                };
                tokio::try_join!(write, read_out, read_err)?;
                child.wait().await
            };
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(Stop::from("cancelled")),
                _ = tokio::time::sleep(Duration::from_secs(DEADLINE_SECS)) => Err(Stop::from("codex_timeout")),
                result = work => result.map_err(|_| Stop::from("codex_io_or_output_cap")),
            }
        };
        if status.is_err() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        let mut stop = status.as_ref().err().map(|e| e.0.clone());
        let mut bounded_text = |bytes: &[u8], cap: usize| {
            let bytes = &bytes[..bytes.len().min(cap)];
            if std::str::from_utf8(bytes).is_err() {
                stop.get_or_insert_with(|| "codex_output_encoding".into());
            }
            let mut text = String::from_utf8_lossy(bytes).into_owned();
            let mut end = text.len().min(cap);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            text
        };
        let stdout = bounded_text(&out, MAX_STDOUT);
        let stderr = bounded_text(&err, MAX_STDERR);
        let transcript = Transcript {
            stdout,
            stderr,
            exit_code: status.as_ref().ok().and_then(|s| s.code()),
            stop,
        };
        Ok(transcript)
    }
}

pub struct Mock;
#[async_trait]
impl Transport for Mock {
    async fn run(&mut self, _: &Value, _: &CancellationToken) -> Result<Transcript> {
        let events = [
            json!({"type":"thread.started","thread_id":"synthetic"}),
            json!({"type":"turn.started"}),
            json!({"type":"item.completed","item":{
                "type":"agent_message","text":"{\"choice\":\"insufficient\"}"}}),
            json!({"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":10}}),
        ];
        Ok(Transcript {
            stdout: events.iter().map(|e| format!("{e}\n")).collect(),
            stderr: String::new(),
            exit_code: Some(0),
            stop: None,
        })
    }
}
