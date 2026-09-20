//! Trusted local method transport. It is not a sandbox or a model tool.
use crate::{
    contract::*,
    evidence::{decode, encoded},
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    process::Stdio,
    sync::{Arc, Mutex as SyncMutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};

const MAX_MESSAGE: usize = 196608;
struct Rpc {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    serial: u64,
    pending: Option<(u64, String)>,
    outgoing: Vec<u8>,
    written: usize,
    incoming: Vec<u8>,
    receipts: Arc<SyncMutex<Vec<Value>>>,
    poisoned: bool,
}
struct Reply {
    result: Option<Value>,
    error: Option<String>,
}
impl Rpc {
    async fn flush(&mut self) -> Result<()> {
        while self.written < self.outgoing.len() {
            let n = self
                .input
                .write(&self.outgoing[self.written..])
                .await
                .map_err(|_| Stop::from("adapter_disconnected"))?;
            require(n > 0, "adapter_disconnected")?;
            self.written += n;
        }
        self.outgoing.clear();
        self.written = 0;
        Ok(())
    }
    fn queue(&mut self, request: Value) -> Result<()> {
        require(self.outgoing.is_empty(), "protocol_write_pending")?;
        self.outgoing = encoded(&request)?;
        self.outgoing.push(b'\n');
        require(self.outgoing.len() <= 32768, "protocol_request_size")
    }
    async fn receive(&mut self) -> Result<Reply> {
        loop {
            let available = self
                .output
                .fill_buf()
                .await
                .map_err(|_| Stop::from("adapter_disconnected"))?;
            require(!available.is_empty(), "adapter_disconnected")?;
            let end = available.iter().position(|b| *b == b'\n');
            let n = end.map_or(available.len(), |i| i + 1);
            require(
                self.incoming.len() + n <= MAX_MESSAGE,
                "protocol_reply_size",
            )?;
            self.incoming.extend_from_slice(&available[..n]);
            self.output.consume(n);
            if end.is_some() {
                break;
            }
        }
        let raw = std::mem::take(&mut self.incoming);
        let reply = decode(&raw)?;
        let (id, method) = self
            .pending
            .as_ref()
            .ok_or_else(|| Stop::from("protocol_unexpected_reply"))?;
        let map = reply
            .as_object()
            .ok_or_else(|| Stop::from("protocol_reply_shape"))?;
        require(
            map.len() == 4
                && reply["version"] == 1
                && reply["id"].as_u64() == Some(*id)
                && map.contains_key("receipts")
                && (map.contains_key("result") != map.contains_key("error")),
            "protocol_reply_shape",
        )?;
        let rows = reply["receipts"]
            .as_array()
            .ok_or_else(|| Stop::from("provider_evidence_size"))?;
        require(
            rows.len() <= 1
                && encoded(rows)?.len() <= 131072
                && (method == "select" || rows.is_empty()),
            "provider_evidence_size",
        )?;
        self.receipts
            .lock()
            .expect("receipt lock")
            .extend(rows.iter().cloned());
        let error = if map.contains_key("error") {
            Some(
                reply["error"]
                    .as_str()
                    .ok_or_else(|| Stop::from("protocol_reply_shape"))?
                    .to_owned(),
            )
        } else {
            None
        };
        self.pending = None;
        Ok(Reply {
            result: map.get("result").cloned(),
            error,
        })
    }
    async fn exchange(&mut self, method: &str, params: Value) -> Result<Value> {
        require(!self.poisoned, "protocol_unavailable")?;
        // A dropped method future may have written part of a request or read part
        // of its reply. Finish that frame, cancel, and drain exactly once before
        // any final evaluation or new method. Never queue another executable call.
        self.flush().await?;
        if let Some((id, _)) = self.pending.clone() {
            self.queue(json!({"version":1,"id":id,"method":"cancel","params":null}))?;
            self.flush().await?;
            self.receive().await?;
        }
        self.serial += 1;
        self.pending = Some((self.serial, method.into()));
        self.queue(json!({"version":1,"id":self.serial,"method":method,"params":params}))?;
        self.flush().await?;
        let reply = self.receive().await?;
        if let Some(error) = reply.error {
            return Err(Stop(error));
        }
        reply.result.ok_or_else(|| "protocol_reply_shape".into())
    }
    async fn call<T: DeserializeOwned>(&mut self, method: &str, params: Value) -> Result<T> {
        let result = self.exchange(method, params).await;
        // A well-formed adapter refusal has consumed its frame. I/O/shape failure
        // leaves pending state and makes further effects unsafe.
        if result.is_err() && self.pending.is_some() {
            self.poisoned = true;
        }
        serde_json::from_value(result?).map_err(|_| "protocol_result_shape".into())
    }
}
#[derive(Clone)]
pub struct ProcessAdapter {
    rpc: Arc<Mutex<Rpc>>,
}
pub struct ProcessProvider {
    rpc: Arc<Mutex<Rpc>>,
    receipts: Arc<SyncMutex<Vec<Value>>>,
}
impl ProcessAdapter {
    pub fn spawn(argv: &[String]) -> Result<(Self, ProcessProvider)> {
        let program = argv.first().ok_or_else(|| Stop::from("adapter_argv"))?;
        let mut child = Command::new(program)
            .args(&argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| Stop::from("adapter_spawn"))?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| Stop::from("adapter_spawn"))?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| Stop::from("adapter_spawn"))?,
        );
        let receipts = Arc::new(SyncMutex::new(vec![]));
        let rpc = Arc::new(Mutex::new(Rpc {
            child,
            input,
            output,
            serial: 0,
            pending: None,
            outgoing: vec![],
            written: 0,
            incoming: vec![],
            receipts: receipts.clone(),
            poisoned: false,
        }));
        Ok((Self { rpc: rpc.clone() }, ProcessProvider { rpc, receipts }))
    }
    pub async fn terminate(&mut self) {
        let mut rpc = self.rpc.lock().await;
        if !matches!(rpc.child.try_wait(), Ok(Some(_))) {
            let _ = rpc.child.kill().await;
        }
    }
}
#[async_trait]
impl Adapter for ProcessAdapter {
    async fn describe(&mut self) -> Result<Descriptor> {
        self.rpc.lock().await.call("describe", Value::Null).await
    }
    async fn reset(&mut self) -> Result<()> {
        self.rpc.lock().await.call("reset", Value::Null).await
    }
    async fn observe(&mut self) -> Result<Frame> {
        self.rpc.lock().await.call("observe", Value::Null).await
    }
    async fn verify(&mut self) -> Result<Descriptor> {
        self.rpc.lock().await.call("verify", Value::Null).await
    }
    async fn execute(&mut self, op: &Value) -> Result<Value> {
        self.rpc.lock().await.call("execute", op.clone()).await
    }
    async fn evaluate(&mut self, phase: &str, op: Option<&Value>) -> Result<Verdict> {
        self.rpc
            .lock()
            .await
            .call("evaluate", json!({"phase":phase,"operation":op}))
            .await
    }
    async fn close(&mut self) -> Result<()> {
        let mut rpc = self.rpc.lock().await;
        rpc.call::<()>("close", Value::Null).await?;
        require(
            rpc.child
                .wait()
                .await
                .map_err(|_| Stop::from("adapter_exit"))?
                .success(),
            "adapter_exit",
        )
    }
}
#[async_trait]
impl Provider for ProcessProvider {
    fn name(&self) -> &str {
        "process-provider"
    }
    fn evidence_limit(&self) -> usize {
        131072
    }
    async fn select(&mut self, request: Value) -> Result<String> {
        self.rpc.lock().await.call("select", request).await
    }
    fn take_evidence(&mut self) -> Vec<Value> {
        std::mem::take(&mut *self.receipts.lock().expect("receipt lock"))
    }
}
