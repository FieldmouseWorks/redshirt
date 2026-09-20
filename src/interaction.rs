//! The existing local decision protocol. No environment authority or game rules.
use crate::{Provider, Result, Stop, evidence, require};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

pub const MAX_MESSAGE: usize = 32768;
pub const MAX_REPLY: usize = 4096;

pub fn choice_tool(candidates: &serde_json::Map<String, Value>) -> Value {
    json!({"name":"choose_action","description":"Choose one currently offered action.",
        "parameters":{"type":"object","properties":{"action_id":{
            "type":"string","enum":candidates.keys().collect::<Vec<_>>() }},
            "required":["action_id"],"additionalProperties":false}})
}

pub fn completion(report: &Value) -> Value {
    json!({"version":1,"type":"done","stop":report["stop"],
        "requests":report["requests"],"attempted_inputs":report["attempted_inputs"],
        "verified":report["final"]["ok"],"cleanup":report["cleanup"],
        "replayable":report["replayable"]})
}

pub struct Interactive<R, W> {
    reader: BufReader<R>,
    writer: W,
    incoming: Vec<u8>,
    outgoing: Vec<u8>,
    written: usize,
    receipt: Option<Value>,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> Interactive<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer,
            incoming: vec![],
            outgoing: vec![],
            written: 0,
            receipt: None,
        }
    }

    // Keep progress outside the future: cancellation must not splice JSON frames.
    async fn flush_pending(&mut self) -> Result<()> {
        while self.written < self.outgoing.len() {
            let n = self
                .writer
                .write(&self.outgoing[self.written..])
                .await
                .map_err(|_| Stop::from("client_disconnected"))?;
            require(n != 0, "client_disconnected")?;
            self.written += n;
        }
        self.writer
            .flush()
            .await
            .map_err(|_| Stop::from("client_disconnected"))?;
        self.outgoing.clear();
        self.written = 0;
        Ok(())
    }

    async fn send(&mut self, message: &Value) -> Result<()> {
        self.flush_pending().await?;
        let mut raw = evidence::encoded(message)?;
        raw.push(b'\n');
        require(raw.len() <= MAX_MESSAGE, "api_message_size")?;
        self.outgoing = raw;
        self.flush_pending().await
    }

    async fn receive(&mut self) -> Result<Value> {
        loop {
            let chunk = self
                .reader
                .fill_buf()
                .await
                .map_err(|_| Stop::from("client_disconnected"))?;
            if chunk.is_empty() {
                return Err(if self.incoming.is_empty() {
                    "client_disconnected"
                } else {
                    "invalid_api_reply"
                }
                .into());
            }
            let newline = chunk.iter().position(|b| *b == b'\n');
            let n = newline.map_or(chunk.len(), |i| i + 1);
            require(self.incoming.len() + n <= MAX_REPLY, "invalid_api_reply")?;
            self.incoming.extend_from_slice(&chunk[..n]);
            self.reader.consume(n);
            if newline.is_some() {
                return evidence::decode(&std::mem::take(&mut self.incoming))
                    .map_err(|_| "invalid_api_reply".into());
            }
        }
    }

    pub async fn finish(&mut self, report: &Value) -> Result<()> {
        self.send(&completion(report)).await
    }
}

#[async_trait]
impl<R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send> Provider for Interactive<R, W> {
    fn name(&self) -> &str {
        "local-jsonl"
    }
    fn evidence_limit(&self) -> usize {
        2048
    }
    async fn select(&mut self, request: Value) -> Result<String> {
        require(self.receipt.is_none(), "provider_busy")?;
        let mut random = [0; 16];
        getrandom::fill(&mut random).map_err(|_| Stop::from("decision_entropy"))?;
        let decision: String = random.iter().map(|b| format!("{b:02x}")).collect();
        // Installed before awaiting, so a dropped future retains its receipt.
        self.receipt = Some(json!({"decision_id":decision,"status":"interrupted"}));
        let result: Result<String> = async {
            let candidates = request["candidates"]
                .as_object()
                .ok_or_else(|| Stop::from("invalid_request"))?;
            let mut message = request
                .as_object()
                .ok_or_else(|| Stop::from("invalid_request"))?
                .clone();
            message.insert("type".into(), "observation".into());
            message.insert("decision_id".into(), decision.clone().into());
            message.insert("tool".into(), choice_tool(candidates));
            self.send(&Value::Object(message)).await?;
            let reply = self.receive().await?;
            let obj = reply
                .as_object()
                .ok_or_else(|| Stop::from("invalid_api_reply"))?;
            require(
                obj.len() == 2
                    && reply["decision_id"].is_string()
                    && reply["action_id"].is_string(),
                "invalid_api_reply",
            )?;
            require(reply["decision_id"] == decision, "stale_decision")?;
            let action = reply["action_id"].as_str().expect("validated string");
            require(candidates.contains_key(action), "unknown_candidate")?;
            Ok(action.into())
        }
        .await;
        let receipt = self.receipt.as_mut().expect("pending receipt");
        match &result {
            Ok(action) => {
                receipt["status"] = "selected".into();
                receipt["action_id"] = action.clone().into();
            }
            Err(error) => receipt["status"] = error.to_string().into(),
        }
        result
    }
    fn take_evidence(&mut self) -> Vec<Value> {
        self.receipt.take().into_iter().collect()
    }
}

/// Async pipe descriptors avoid an uncancellable blocking stdin read at shutdown.
#[cfg(unix)]
pub fn stdio()
-> Result<Interactive<tokio::net::unix::pipe::Receiver, tokio::net::unix::pipe::Sender>> {
    use std::os::fd::AsFd;
    use tokio::net::unix::pipe::{Receiver, Sender};
    let incoming = std::io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|_| Stop::from("stdio_requires_pipes"))?;
    let outgoing = std::io::stdout()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|_| Stop::from("stdio_requires_pipes"))?;
    Ok(Interactive::new(
        Receiver::from_owned_fd(incoming).map_err(|_| Stop::from("stdio_requires_pipes"))?,
        Sender::from_owned_fd(outgoing).map_err(|_| Stop::from("stdio_requires_pipes"))?,
    ))
}
