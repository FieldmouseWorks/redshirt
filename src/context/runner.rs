use super::*;
use crate::{CancellationToken, Provider, evidence::decode};
use async_trait::async_trait;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    time::Instant,
};

const MAX_ARTIFACT: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    case: String,
    phase: String,
    elapsed_ms: f64,
    error: Option<String>,
    receipt: Option<Value>,
}

fn write_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = encoded(value)?;
    require(bytes.len() <= MAX_ARTIFACT, "context_artifact_size")?;
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .and_then(|mut file| {
            file.write_all(&bytes)?;
            file.sync_all()
        })
        .map_err(|_| Stop::from("context_evidence_io"))
}

fn checkpoint(output: &Path, report: &Value) -> Result<()> {
    write_new(&output.join("report.pending.json"), report)?;
    fs::rename(
        output.join("report.pending.json"),
        output.join("report.json"),
    )
    .map_err(|_| Stop::from("context_evidence_io"))
}

fn phases(index: usize) -> [&'static str; 3] {
    if index.is_multiple_of(2) {
        ["baseline", "selection", "treatment"]
    } else {
        ["selection", "treatment", "baseline"]
    }
}

fn stage_request(
    manifest: &Manifest,
    case: &Case,
    phase: &str,
    scores: Option<&Answers>,
) -> Result<(Value, Questions, Vec<String>)> {
    let selected = match phase {
        "baseline" => select(manifest, case, None)?,
        "selection" => case.chunks.iter().map(|c| c.id.clone()).collect(),
        "treatment" => select(
            manifest,
            case,
            Some(scores.ok_or_else(|| Stop::from("context_missing_scores"))?),
        )?,
        _ => return Err("context_phase".into()),
    };
    let questions = if phase == "selection" {
        scoring_questions(case)
    } else {
        diagnosis_question(case)
    };
    Ok((state(manifest, case, &selected), questions, selected))
}

/// The oracle never reaches this function or the provider; it is used only by
/// independent analysis after the recorded responses have been collected.
async fn collect<T: jev::Transport>(
    manifest: &Manifest,
    provider: &mut jev::Jev<T>,
    output: &Path,
    cancel: &CancellationToken,
    report: &mut Value,
) -> Result<Vec<Call>> {
    let mut calls = Vec::new();
    let mut events = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("calls.jsonl"))
        .map_err(|_| Stop::from("context_evidence_io"))?;
    let mut used = 0;
    for (index, case) in manifest.cases.iter().enumerate() {
        let mut scores = None;
        for phase in phases(index) {
            if cancel.is_cancelled() {
                report["error"] = "cancelled".into();
                return Ok(calls);
            }
            let (state, questions, _) = stage_request(manifest, case, phase, scores.as_ref())?;
            // Reserve and durably checkpoint before a possible dispatch. An
            // interrupted call retains its reservation; there is no resumption.
            require(
                calls.len() < manifest.limits.max_calls as usize,
                "context_call_budget",
            )?;
            let reserved = calls.len() + 1;
            require(
                reserved as f64 * per_call_usd() <= manifest.limits.max_reserved_usd,
                "context_cost_budget",
            )?;
            report["reserved_calls"] = reserved.into();
            report["reserved_usd"] = json!(reserved as f64 * per_call_usd());
            report["pending"] = json!({"case":case.id,"phase":phase});
            checkpoint(output, report)?;
            let started = Instant::now();
            let result = tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(Stop::from("cancelled")),
                result = provider.ask(state, questions) => result,
            };
            let mut receipts = provider.take_evidence();
            require(receipts.len() <= 1, "context_receipt_count")?;
            let call = Call {
                case: case.id.clone(),
                phase: phase.into(),
                elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
                error: result.as_ref().err().map(|e| e.0.clone()),
                receipt: receipts.pop(),
            };
            let mut bytes = encoded(&call)?;
            bytes.push(b'\n');
            require(
                bytes.len() <= 262144 && used + bytes.len() <= MAX_ARTIFACT,
                "context_evidence_budget",
            )?;
            events
                .write_all(&bytes)
                .and_then(|_| events.sync_all())
                .map_err(|_| Stop::from("context_evidence_io"))?;
            used += bytes.len();
            calls.push(call);
            report["pending"] = Value::Null;
            report["transport_attempts"] = provider.calls().into();
            checkpoint(output, report)?;
            match result {
                Ok(answers) if phase == "selection" => scores = Some(answers),
                Ok(_) => {}
                Err(error) => {
                    report["error"] = error.0.into();
                    return Ok(calls);
                }
            }
        }
    }
    Ok(calls)
}

fn analyze(manifest: &Manifest, oracle: &Oracle, calls: &[Call]) -> Result<Value> {
    require(
        calls.len() <= manifest.cases.len() * 3,
        "context_call_count",
    )?;
    let mut rows = Vec::new();
    let mut costs = Vec::new();
    let mut failures = Vec::new();
    let mut position = 0;
    let mut attempts = 0;
    for (index, case) in manifest.cases.iter().enumerate() {
        let mut scores = None;
        let mut selection_ms = 0.0;
        for phase in phases(index) {
            let Some(call) = calls.get(position) else {
                break;
            };
            position += 1;
            require(
                call.case == case.id
                    && call.phase == phase
                    && call.elapsed_ms.is_finite()
                    && call.elapsed_ms >= 0.0,
                "context_call_order",
            )?;
            let (state, questions, selected) =
                stage_request(manifest, case, phase, scores.as_ref())?;
            let expected = request(state.clone(), &questions);
            let mut input_tokens = None;
            let answers = if let Some(receipt) = &call.receipt {
                attempts += 1;
                require(
                    receipt["request"] == expected
                        && receipt["request_sha256"] == sha256(&encoded(&expected)?)
                        && receipt["model"] == jev::MODEL
                        && receipt["call"] == attempts
                        && receipt["reserved_input_tokens"] == jev::MAX_TOKENS
                        && receipt["reserved_usd"] == json!(per_call_usd())
                        && receipt["question_count"] == questions.len(),
                    "context_receipt_binding",
                )?;
                if call.error.is_none() {
                    require(receipt["outcome"] == "accepted", "context_receipt_outcome")?;
                    let raw = receipt["response"]
                        .as_str()
                        .ok_or_else(|| Stop::from("context_response"))?;
                    let answers = jev::recorded_answers(raw.as_bytes(), &questions)?;
                    let response = decode(raw.as_bytes())?;
                    require(
                        receipt["usage"] == response["usage"],
                        "context_usage_binding",
                    )?;
                    input_tokens = response["usage"]["input_tokens"].as_u64();
                    Some(answers)
                } else {
                    require(
                        receipt["outcome"] == "failed" || receipt["outcome"] == "interrupted",
                        "context_receipt_outcome",
                    )?;
                    None
                }
            } else {
                require(call.error.is_some(), "context_missing_receipt")?;
                None
            };
            costs.push(json!({"case":case.id,"split":case.split,
                "arm":if phase == "baseline" {"baseline"} else {"treatment"},
                "phase":phase,"elapsed_ms":call.elapsed_ms,"input_tokens":input_tokens,
                "estimated_usd":input_tokens.map(|n| n as f64*jev::INPUT_USD_PER_MILLION/1e6),
                "unknown_usage":call.receipt.is_some() && input_tokens.is_none()}));
            if let Some(error) = &call.error {
                require(position == calls.len(), "context_calls_after_failure")?;
                failures.push(json!({"case":case.id,"phase":phase,"error":error}));
                break;
            }
            let answers = answers.ok_or_else(|| Stop::from("context_missing_answers"))?;
            if phase == "selection" {
                scores = Some(answers);
                selection_ms = call.elapsed_ms;
            } else {
                let Answer::Choice {
                    choice, confidence, ..
                } = &answers["diagnosis"]
                else {
                    return Err("context_diagnosis_type".into());
                };
                let truth = &oracle.cases[&case.id];
                rows.push(json!({"case":case.id,"split":case.split,"arm":phase,
                    "choice":choice,"correct":choice == &truth.diagnosis,"confidence":confidence,
                    "selected":selected,"missing_essential":truth.essential.iter().filter(|id| !selected.contains(id)).collect::<Vec<_>>(),
                    "context_bytes":encoded(&state)?.len(),"context_sha256":sha256(&encoded(&state)?),
                    "mandatory_sha256":sha256(&encoded(&state["mandatory"])?),
                    "diagnostic_ms":call.elapsed_ms,"selection_ms":if phase == "treatment" {selection_ms} else {0.0},
                    "provider_elapsed_ms":call.elapsed_ms+if phase == "treatment" {selection_ms} else {0.0}}));
            }
        }
    }
    let mut summary = Vec::new();
    for split in ["calibration", "held_out"] {
        for arm in ["baseline", "treatment"] {
            let rows: Vec<_> = rows
                .iter()
                .filter(|r| r["split"] == split && r["arm"] == arm)
                .collect();
            let costs: Vec<_> = costs
                .iter()
                .filter(|r| r["split"] == split && r["arm"] == arm)
                .collect();
            let planned = manifest
                .cases
                .iter()
                .filter(|c| json!(c.split) == split)
                .count();
            summary.push(json!({"split":split,"arm":arm,"planned":planned,"graded":rows.len(),
                "correct":rows.iter().filter(|r| r["correct"] == true).count(),
                "insufficient":rows.iter().filter(|r| r["choice"] == "insufficient").count(),
                "ungraded":planned-rows.len(),"calls":costs.len(),
                "missing_essential":rows.iter().map(|r| r["missing_essential"].as_array().map_or(0,Vec::len)).sum::<usize>(),
                "input_tokens":costs.iter().filter_map(|r| r["input_tokens"].as_u64()).sum::<u64>(),
                "estimated_usd":costs.iter().filter_map(|r| r["estimated_usd"].as_f64()).sum::<f64>(),
                "provider_elapsed_ms":costs.iter().filter_map(|r| r["elapsed_ms"].as_f64()).sum::<f64>(),
                "unknown_usage_calls":costs.iter().filter(|r| r["unknown_usage"] == true).count()}));
        }
    }
    Ok(
        json!({"rows":rows,"calls":costs,"failures":failures,"summary":summary,
        "transport_attempts":attempts,"complete":calls.len()==manifest.cases.len()*3 && failures.is_empty(),
        "cache_usage":"not_exposed_by_provider","billed_usd":null}),
    )
}

pub async fn campaign<T: jev::Transport>(
    manifest: Manifest,
    oracle: Oracle,
    mut provider: jev::Jev<T>,
    output: &Path,
    live: bool,
    cancel: &CancellationToken,
) -> Result<Value> {
    preflight(&manifest, &oracle)?;
    require(
        provider.calls() == 0 && provider.name() == if live { "jev-live" } else { "jev-mock" },
        "context_provider_mode",
    )?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(output)
        .map_err(|_| Stop::from("output_exists_or_unavailable"))?;
    write_new(&output.join("manifest.json"), &manifest)?;
    write_new(&output.join("oracle.json"), &oracle)?;
    let started = Instant::now();
    let mut report = json!({"version":1,"mode":if live {"live"} else {"mock"},
        "quality_evidence":live,"model":jev::MODEL,"manifest_sha256":manifest.digest()?,
        "oracle_sha256":sha256(&encoded(&oracle)?),"complete":false,"error":null,
        "reserved_calls":0,"reserved_usd":0.0,"pending":null,"transport_attempts":0});
    checkpoint(output, &report)?;
    let collected = collect(&manifest, &mut provider, output, cancel, &mut report).await;
    match collected {
        Ok(calls) => {
            let analysis = analyze(&manifest, &oracle, &calls)?;
            report["complete"] = analysis["complete"].clone();
            report["analysis"] = analysis;
            report["live_calls"] = if live { provider.calls() } else { 0 }.into();
            report["wall_elapsed_ms"] = json!(started.elapsed().as_secs_f64() * 1000.0);
            checkpoint(output, &report)?;
            Ok(report)
        }
        Err(error) => {
            report["error"] = error.0.clone().into();
            checkpoint(output, &report)?;
            Err(error)
        }
    }
}

pub fn replay(output: &Path) -> Result<Value> {
    let (manifest, oracle) =
        read_inputs(&output.join("manifest.json"), &output.join("oracle.json"))?;
    let report = load(&output.join("report.json"), MAX_ARTIFACT)?;
    require(
        report["version"] == 1
            && report["model"] == jev::MODEL
            && report["manifest_sha256"] == manifest.digest()?
            && report["oracle_sha256"] == sha256(&encoded(&oracle)?)
            && report["pending"].is_null(),
        "context_replay_identity",
    )?;
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(output.join("calls.jsonl"))
        .map_err(|_| Stop::from("context_evidence_io"))?
        .take(MAX_ARTIFACT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Stop::from("context_evidence_io"))?;
    require(
        bytes.len() <= MAX_ARTIFACT && (bytes.is_empty() || bytes.ends_with(b"\n")),
        "context_replay_size",
    )?;
    let mut calls = Vec::new();
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        require(calls.len() < MAX_CALLS as usize, "context_replay_calls")?;
        calls.push(
            serde_json::from_value(decode(line)?)
                .map_err(|_| Stop::from("context_replay_schema"))?,
        );
    }
    let analysis = analyze(&manifest, &oracle, &calls)?;
    let live = match report["mode"].as_str() {
        Some("live") => true,
        Some("mock") => false,
        _ => return Err("context_replay_mode".into()),
    };
    require(
        report["quality_evidence"] == live
            && report["reserved_calls"] == calls.len()
            && report["reserved_usd"] == json!(calls.len() as f64 * per_call_usd())
            && report["transport_attempts"] == analysis["transport_attempts"]
            && report["live_calls"]
                == if live {
                    analysis["transport_attempts"].clone()
                } else {
                    json!(0)
                },
        "context_replay_accounting",
    )?;
    require(
        report["analysis"] == analysis && report["complete"] == analysis["complete"],
        "context_replay_mismatch",
    )?;
    Ok(
        json!({"verified":true,"provider_calls":0,"complete":report["complete"],"analysis":analysis}),
    )
}

/// Manufactured answers exercise the protocol only. They never consult labels.
pub struct MockTransport;
#[async_trait]
impl jev::Transport for MockTransport {
    async fn post(&mut self, body: Vec<u8>) -> Result<(u16, Vec<u8>)> {
        let body = decode(&body)?;
        let questions: Questions = serde_json::from_value(body["questions"].clone())
            .map_err(|_| Stop::from("mock_questions"))?;
        let answers: BTreeMap<_,_> = questions.into_iter().map(|(id, q)| {
            let answer = match q {
                Question::Choice { criteria, .. } => {
                    let choice = criteria.keys().next().expect("validated criteria").clone();
                    let probabilities: BTreeMap<_,_> = criteria.keys().map(|key|
                        (key.clone(), if key == &choice {1.0} else {0.0})).collect();
                    json!({"type":"choice","choice":choice,"probabilities":probabilities,"confidence":1.0})
                },
                Question::Score { criteria, .. } => {
                    let legend: BTreeMap<_,_> = criteria.into_iter().enumerate().map(|(i,s)|(i.to_string(),s)).collect();
                    json!({"type":"score","score":0.0,"legend":legend,"probabilities":{"0":1.0},"confidence":1.0})
                },
                Question::Noul { .. } => json!({"type":"noul","noul":0.0}),
            };
            (id, answer)
        }).collect();
        Ok((
            200,
            encoded(&json!({"model":jev::MODEL,"answers":answers,
            "usage":{"input_tokens":100,"output_tokens":10}}))?,
        ))
    }
}
