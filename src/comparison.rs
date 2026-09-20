//! Bounded comparisons; consumers own the baseline and independent task labels.
use crate::{
    evidence::{decode, encoded, load, sha256},
    jev,
    process::ProcessAdapter,
    *,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path, time::Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub split: Split,
    pub adapter: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    Calibration,
    HeldOut,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub cases: Vec<Case>,
    pub limits: Limits,
    pub jev: jev::Config,
    pub max_live_calls: u32,
    pub max_reserved_usd: f64,
    pub thresholds: Vec<f64>,
}
impl Manifest {
    pub fn validate(&self) -> Result<()> {
        self.limits.validate()?;
        self.jev.validate()?;
        require(
            self.version == 1 && (2..=8).contains(&self.cases.len()),
            "comparison_cases",
        )?;
        let mut ids = BTreeSet::new();
        for case in &self.cases {
            require(
                !case.id.is_empty()
                    && case.id.len() <= 40
                    && case
                        .id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                    && ids.insert(&case.id)
                    && !case.adapter.is_empty()
                    && case.adapter.len() <= 24
                    && case
                        .adapter
                        .iter()
                        .all(|s| !s.is_empty() && s.len() <= 4096),
                "comparison_case",
            )?;
        }
        require(
            self.cases.iter().any(|c| c.split == Split::Calibration)
                && self.cases.iter().any(|c| c.split == Split::HeldOut),
            "comparison_splits",
        )?;
        require(
            self.jev.policy.min_confidence.is_none(),
            "comparison_collect_without_floor",
        )?;
        require(
            self.limits.requests == self.jev.request_limit,
            "comparison_request_limits",
        )?;
        let calls = self.cases.len() as u32 * self.jev.request_limit;
        require(
            calls <= self.max_live_calls
                && self.max_live_calls <= 96
                && self.max_reserved_usd.is_finite()
                && self.max_reserved_usd > 0.0
                && self.max_reserved_usd <= 1.0
                && self.reservation() <= self.max_reserved_usd,
            "comparison_allowance",
        )?;
        require(
            !self.thresholds.is_empty()
                && self.thresholds.len() <= 11
                && self.thresholds[0] == 0.0
                && self.thresholds.iter().all(|x| decision::probability(*x))
                && self.thresholds.windows(2).all(|w| w[0] < w[1]),
            "comparison_thresholds",
        )
    }
    pub fn reservation(&self) -> f64 {
        self.cases.len() as f64 * self.jev.request_limit as f64 * per_call_usd()
    }
}
fn per_call_usd() -> f64 {
    jev::MAX_TOKENS as f64 * jev::INPUT_USD_PER_MILLION / 1e6
}

/// Reserved consumer check namespace. Absence is unknown, never success.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCheck {
    pub complete: bool,
    pub useful_action: Option<bool>,
}
fn task(verdict: &Value) -> Result<TaskCheck> {
    serde_json::from_value(verdict["checks"]["comparison"].clone())
        .map_err(|_| Stop::from("comparison_task_check"))
}

/// Checks the identical first request before a model can receive it. Timings
/// include the full selection future; a dropped future remains an unknown call.
pub struct MatchedProvider {
    pub inner: Box<dyn Provider>,
    pub expected_first: Option<String>,
    pub first: Option<String>,
    pub elapsed_ms: Vec<f64>,
    active: Option<Instant>,
}
impl MatchedProvider {
    pub fn new(inner: Box<dyn Provider>, expected_first: Option<String>) -> Self {
        Self {
            inner,
            expected_first,
            first: None,
            elapsed_ms: vec![],
            active: None,
        }
    }
}
#[async_trait]
impl Provider for MatchedProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn evidence_limit(&self) -> usize {
        self.inner.evidence_limit()
    }
    async fn select(&mut self, request: Value) -> Result<String> {
        if self.first.is_none() {
            let digest = sha256(&encoded(&request)?);
            self.first = Some(digest.clone());
            require(
                self.expected_first.as_ref().is_none_or(|e| e == &digest),
                "comparison_initial_mismatch",
            )?;
        }
        self.active = Some(Instant::now());
        let result = self.inner.select(request).await;
        self.elapsed_ms.push(
            self.active
                .take()
                .expect("selection clock")
                .elapsed()
                .as_secs_f64()
                * 1000.0,
        );
        result
    }
    fn take_evidence(&mut self) -> Vec<Value> {
        if let Some(started) = self.active.take() {
            self.elapsed_ms
                .push(started.elapsed().as_secs_f64() * 1000.0);
        }
        self.inner.take_evidence()
    }
}

/// Local protocol exercise only: the baseline supplies the action, auxiliary
/// distributions are manufactured. Never report these as model judgments.
struct MockTransport(Box<dyn Provider>);
#[async_trait]
impl jev::Transport for MockTransport {
    async fn post(&mut self, body: Vec<u8>) -> Result<(u16, Vec<u8>)> {
        let body = decode(&body)?;
        let action = self.0.select(body["state"].clone()).await?;
        let mut answers = serde_json::Map::new();
        for (id, q) in body["questions"]
            .as_object()
            .ok_or_else(|| Stop::from("mock_questions"))?
        {
            let answer = match q["type"].as_str() {
                Some("choice") => {
                    let criteria = q["criteria"]
                        .as_object()
                        .ok_or_else(|| Stop::from("mock_criteria"))?;
                    let chosen = if id == "action" {
                        action.as_str()
                    } else {
                        criteria
                            .keys()
                            .next()
                            .ok_or_else(|| Stop::from("mock_criteria"))?
                    };
                    let probabilities: serde_json::Map<String, Value> = criteria
                        .keys()
                        .map(|key| (key.clone(), json!(if key == chosen { 1.0 } else { 0.0 })))
                        .collect();
                    json!({"type":"choice","choice":chosen,"probabilities":probabilities,"confidence":1.0})
                }
                Some("score") => {
                    let criteria = q["criteria"]
                        .as_array()
                        .ok_or_else(|| Stop::from("mock_criteria"))?;
                    let legend: serde_json::Map<String, Value> = criteria
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (i.to_string(), v.clone()))
                        .collect();
                    json!({"type":"score","score":0.0,"legend":legend,"probabilities":{"0":1.0},"confidence":1.0})
                }
                Some("noul") => json!({"type":"noul","noul":0.0}),
                _ => return Err("mock_question_type".into()),
            };
            answers.insert(id.clone(), answer);
        }
        Ok((
            200,
            encoded(
                &json!({"model":jev::MODEL,"answers":answers,"usage":{"input_tokens":100,"output_tokens":10}}),
            )?,
        ))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    pub confidence: Option<f64>,
    pub useful: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct Measurement {
    pub valid: bool,
    pub task_complete: bool,
    pub checked_actions: usize,
    pub wasted_actions: usize,
    pub ungraded_attempts: usize,
    pub decisions: Vec<Decision>,
    pub provider_calls: usize,
    pub reserved_usd: f64,
    pub estimated_usd: f64,
    pub unknown_usage_calls: usize,
    pub episode_seconds: f64,
}

pub fn measure(report: &Value, events: &[Value], live: bool) -> Measurement {
    let final_task = task(&report["final"]);
    let evaluations = report["evaluations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let labels: Vec<_> = evaluations.iter().map(task).collect();
    let checked_actions = labels
        .iter()
        .filter(|t| t.as_ref().is_ok_and(|t| t.useful_action.is_some()))
        .count();
    let valid = final_task.is_ok() && checked_actions == evaluations.len();
    let verified = report["setup_verified"] == true
        && report["identity_stable"] == true
        && report["final"]["ok"] == true
        && report["cleanup"] == true
        && evaluations.iter().all(|v| v["ok"] == true)
        && !matches!(
            report["stop"].as_str(),
            Some("identity_changed" | "environment_changed" | "comparison_initial_mismatch")
        );
    let complete = valid && verified && final_task.is_ok_and(|t| t.complete);
    let mut result = Measurement {
        valid,
        task_complete: complete,
        checked_actions,
        wasted_actions: labels
            .iter()
            .filter(|t| t.as_ref().is_ok_and(|t| t.useful_action == Some(false)))
            .count(),
        ungraded_attempts: report["operations"]
            .as_array()
            .map_or(0, Vec::len)
            .saturating_sub(checked_actions),
        decisions: vec![],
        provider_calls: 0,
        reserved_usd: 0.0,
        estimated_usd: 0.0,
        unknown_usage_calls: 0,
        episode_seconds: report["elapsed_seconds"].as_f64().unwrap_or(0.0),
    };
    let mut pending = None;
    for event in events {
        let data = &event["data"];
        match event["event"].as_str() {
            Some("provider_receipt") if data["model"] == jev::MODEL => {
                result.provider_calls += 1;
                if live {
                    result.reserved_usd += data["reserved_usd"].as_f64().unwrap_or(per_call_usd());
                    result.estimated_usd += data["estimated_usd"].as_f64().unwrap_or(0.0);
                    result.unknown_usage_calls += usize::from(!data["estimated_usd"].is_number());
                }
                result.decisions.push(Decision {
                    confidence: data["policy"]["confidence"].as_f64(),
                    useful: None,
                });
                pending = Some(result.decisions.len() - 1);
            }
            Some("decision") if data["candidate_id"] == "stop" => {
                if let Some(i) = pending.take() {
                    // Unknown final checks cannot grade a stop as correct or wrong.
                    if valid && verified {
                        result.decisions[i].useful = Some(complete);
                    }
                }
            }
            Some("checked") => {
                if let Some(i) = pending.take() {
                    result.decisions[i].useful = task(data)
                        .ok()
                        .and_then(|t| t.useful_action)
                        .map(|useful| useful && data["ok"] == true);
                }
            }
            _ => (),
        }
    }
    result
}

/// Observed-decision coverage only. Later states would change after abstention;
/// these tables cannot predict completion of a policy-controlled episode.
pub fn thresholds(rows: &[(Split, Vec<Decision>)], split: Split, floors: &[f64]) -> Vec<Value> {
    floors
        .iter()
        .map(|floor| {
            let (mut accepted, mut mistakes, mut abstained, mut useful_abstained, mut unknown) =
                (0, 0, 0, 0, 0);
            for (_, decisions) in rows.iter().filter(|(s, _)| *s == split) {
                for decision in decisions {
                    match (decision.confidence, decision.useful) {
                        (Some(confidence), Some(useful)) if confidence >= *floor => {
                            accepted += 1;
                            mistakes += usize::from(!useful);
                        }
                        (Some(_), Some(useful)) => {
                            abstained += 1;
                            useful_abstained += usize::from(useful);
                        }
                        _ => unknown += 1,
                    }
                }
            }
            json!({"floor":floor,"accepted":accepted,"accepted_mistakes":mistakes,
            "abstained":abstained,"useful_abstained":useful_abstained,"ungraded":unknown})
        })
        .collect()
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    // Output directory is newly created and never resumed. Atomic checkpoints
    // leave the previous reservation readable if a later write is interrupted.
    let temp = path.with_extension("tmp");
    fs::write(&temp, encoded(value)?).map_err(|_| Stop::from("comparison_output"))?;
    fs::rename(temp, path).map_err(|_| Stop::from("comparison_output"))
}
fn events(path: &Path, limit: usize) -> Result<Vec<Value>> {
    use std::io::Read;
    let mut bytes = vec![];
    fs::File::open(path)
        .map_err(|_| Stop::from("comparison_evidence"))?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Stop::from("comparison_evidence"))?;
    require(bytes.len() <= limit, "comparison_evidence_size")?;
    bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(decode)
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Mock,
    Live,
}
fn live_provider(config: jev::Config, key: &str) -> Result<Box<dyn Provider>> {
    #[cfg(feature = "jev-http")]
    {
        Ok(Box::new(jev::Jev::live(key.to_owned(), config)?))
    }
    #[cfg(not(feature = "jev-http"))]
    {
        let _ = (config, key);
        Err("jev_http_feature_required".into())
    }
}

pub async fn campaign(
    manifest: Manifest,
    output: &Path,
    mode: Mode,
    cancel: &CancellationToken,
) -> Result<Value> {
    manifest.validate()?;
    let key = if mode == Mode::Live {
        let key =
            std::env::var("TYPESAFE_API_KEY").map_err(|_| Stop::from("missing_or_invalid_key"))?;
        // Refuse unsupported builds/bad credentials before starting any adapter.
        let _ = live_provider(manifest.jev.clone(), &key)?;
        Some(key)
    } else {
        None
    };
    fs::create_dir(output).map_err(|_| Stop::from("comparison_output_exists_or_unavailable"))?;
    write_json(&output.join("manifest.json"), &manifest)?;
    let mut summary = json!({"version":1,"mode":if mode == Mode::Live { "live" } else { "mock" },
        "complete":false,"case_reservations":0,"reserved_calls":0,"reserved_usd":0.0,"runs":[],
        "manifest_sha256":sha256(&encoded(&manifest)?),"threshold_claim":"observed decisions only; no counterfactual completion or calibrated probability"});
    write_json(&output.join("comparison.json"), &summary)?;
    let mut baselines: Vec<(String, Value)> = vec![];
    let mut rows = vec![];
    // All baseline cases run first, so missing consumer checks stop before live use.
    for candidate_arm in [false, true] {
        for (index, case) in manifest.cases.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err("cancelled".into());
            }
            let arm = if !candidate_arm {
                "baseline"
            } else if mode == Mode::Live {
                "jev"
            } else {
                "jev-mock"
            };
            let folder = output.join(format!("{}-{arm}", case.id));
            let (mut adapter, baseline) =
                ProcessAdapter::spawn_without_env(&case.adapter, &["TYPESAFE_API_KEY"])?;
            let result = async {
                if candidate_arm {
                    let descriptor = tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        adapter.describe(),
                    )
                    .await
                    .map_err(|_| Stop::from("operation_timeout"))??;
                    require(
                        descriptor.identity == baselines[index].1,
                        "comparison_identity_mismatch",
                    )?;
                }
                let inner: Box<dyn Provider> = if !candidate_arm {
                    Box::new(baseline)
                } else if let Some(key) = &key {
                    live_provider(manifest.jev.clone(), key)?
                } else {
                    Box::new(jev::Jev::new(
                        MockTransport(Box::new(baseline)),
                        manifest.jev.clone(),
                    )?)
                };
                let expected = if candidate_arm {
                    Some(baselines[index].0.clone())
                } else {
                    None
                };
                let mut provider = MatchedProvider::new(inner, expected);
                if candidate_arm && mode == Mode::Live {
                    let n = index as u32 + 1;
                    // Entire-case reservation persists before the first possible dispatch;
                    // interrupted calls are never refunded or retried.
                    summary["case_reservations"] = n.into();
                    summary["reserved_calls"] = (n * manifest.jev.request_limit).into();
                    summary["reserved_usd"] =
                        (n as f64 * manifest.jev.request_limit as f64 * per_call_usd()).into();
                    write_json(&output.join("comparison.json"), &summary)?;
                }
                let report = run(
                    &mut adapter,
                    Some(&mut provider),
                    None,
                    &folder,
                    manifest.limits.clone(),
                    cancel,
                )
                .await?;
                let events = events(&folder.join("events.jsonl"), manifest.limits.evidence_bytes)?;
                let measurement = measure(&report, &events, mode == Mode::Live && candidate_arm);
                let first = provider.first.clone();
                let row = json!({"case":case.id,"split":case.split,"arm":arm,"stop":report["stop"],
                    "identity":report["identity"],"initial_request_sha256":first,
                    "requests":report["requests"],"attempted_inputs":report["attempted_inputs"],
                    "selection_ms":provider.elapsed_ms,"measurement":measurement});
                summary["runs"].as_array_mut().expect("runs").push(row);
                write_json(&output.join("comparison.json"), &summary)?;
                require(
                    measurement.valid && first.is_some(),
                    "comparison_invalid_measurement",
                )?;
                if !candidate_arm {
                    baselines.push((
                        first.expect("measured first request"),
                        report["identity"].clone(),
                    ));
                } else {
                    rows.push((case.split, measurement.decisions));
                }
                require(
                    report["stop"] != "comparison_initial_mismatch",
                    "comparison_initial_mismatch",
                )
            }
            .await;
            adapter.terminate().await;
            result?;
        }
    }
    summary["complete"] = true.into();
    summary["calibration"] = json!(thresholds(&rows, Split::Calibration, &manifest.thresholds));
    summary["held_out"] = json!(thresholds(&rows, Split::HeldOut, &manifest.thresholds));
    write_json(&output.join("comparison.json"), &summary)?;
    Ok(summary)
}

pub fn read_manifest(path: &Path) -> Result<Manifest> {
    serde_json::from_value(load(path, 65536)?).map_err(|_| Stop::from("comparison_manifest"))
}
