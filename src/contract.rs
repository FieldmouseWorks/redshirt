use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop(pub String);
pub type Result<T> = std::result::Result<T, Stop>;
impl From<&str> for Stop {
    fn from(code: &str) -> Self {
        Self(code.into())
    }
}
impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Stop {}
pub fn require(ok: bool, code: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(code.into()) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub inputs: u32,
    pub seconds: f64,
    pub operation_seconds: f64,
    pub final_seconds: f64,
    pub evidence_bytes: usize,
    pub captures: u32,
    pub requests: u32,
    pub no_progress: u32,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            inputs: 24,
            seconds: 180.,
            operation_seconds: 10.,
            final_seconds: 10.,
            evidence_bytes: 8 * 1024 * 1024,
            captures: 0,
            requests: 24,
            no_progress: 3,
        }
    }
}
impl Limits {
    pub fn validate(&self) -> Result<()> {
        require(
            (1..=24).contains(&self.inputs)
                && (1..=24).contains(&self.requests)
                && (1..=3).contains(&self.no_progress)
                && self.seconds.is_finite()
                && self.seconds > 2. * self.final_seconds
                && self.seconds <= 180.
                && self.operation_seconds > 0.
                && self.operation_seconds <= 10.
                && self.final_seconds > 0.
                && self.final_seconds <= 10.
                && (262144..=8 * 1024 * 1024).contains(&self.evidence_bytes)
                && self.captures == 0,
            "invalid_limits",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub identity: Value,
    pub setup_mode: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub environment: String,
    pub epoch: String,
    pub guard: String,
    pub view: Value,
    pub ready: bool,
    pub terminal: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub description: String,
    pub operation: Value,
    pub inputs: u32,
    pub needs_ready: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub observation: Observation,
    pub candidates: Vec<Candidate>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verdict {
    pub ok: bool,
    pub progress: bool,
    pub checks: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub operation: Value,
    pub view: String,
    pub verdict: Verdict,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    pub version: u32,
    pub identity: Value,
    pub complete: bool,
    pub steps: Vec<Step>,
}

/// Dropping an in-process method must stop its work. Process adapters must cancel
/// and drain any pending RPC before running the next method (including final).
#[async_trait]
pub trait Adapter: Send {
    async fn describe(&mut self) -> Result<Descriptor>;
    async fn reset(&mut self) -> Result<()>;
    async fn observe(&mut self) -> Result<Frame>;
    async fn verify(&mut self) -> Result<Descriptor>;
    async fn execute(&mut self, operation: &Value) -> Result<Value>;
    async fn evaluate(&mut self, phase: &str, operation: Option<&Value>) -> Result<Verdict>;
    async fn close(&mut self) -> Result<()>;
}
#[async_trait]
pub trait Provider: Send {
    fn name(&self) -> &str;
    fn evidence_limit(&self) -> usize {
        0
    }
    async fn select(&mut self, request: Value) -> Result<String>;
    fn take_evidence(&mut self) -> Vec<Value> {
        vec![]
    }
}
pub struct Scripted(pub std::collections::VecDeque<String>);
#[async_trait]
impl Provider for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }
    async fn select(&mut self, _: Value) -> Result<String> {
        Ok(self.0.pop_front().unwrap_or_else(|| "stop".into()))
    }
}
