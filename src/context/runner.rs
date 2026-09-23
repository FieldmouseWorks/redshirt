use super::*;
use crate::{CancellationToken, Provider, evidence::decode};
use async_trait::async_trait;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    time::Instant,
};

const LEGACY_MAX_ARTIFACT: usize = 4 * 1024 * 1024;
const MAX_ARTIFACT: usize = 8 * 1024 * 1024;
const LEGACY_MAX_CALL: usize = 262144;
const MAX_CALL: usize = 896 * 1024;
// A canonical JSON string expands by at most six bytes per input byte. The
// remaining 32 KiB bounds receipt fields, answer policy and call bookkeeping.
const RECEIPT_OVERHEAD: usize = 32 * 1024;

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

pub(super) fn evidence_reservation(manifest: &Manifest) -> Result<Value> {
    let mut total = 0;
    let mut largest = 0;
    for case in &manifest.cases {
        let selected = case
            .chunks
            .iter()
            .map(|chunk| chunk.id.clone())
            .collect::<Vec<_>>();
        let full = state(manifest, case, &selected);
        let jev_selection = encoded(&request(full.clone(), &scoring_questions(case)))?.len();
        let jev_diagnosis = encoded(&request(full.clone(), &diagnosis_question(case)))?.len();
        let codex_diagnosis = manifest
            .diagnostic
            .as_ref()
            .map(|profile| {
                codex::request(profile, full.clone(), &case.diagnoses)
                    .and_then(|body| encoded(&body).map(|bytes| bytes.len()))
            })
            .transpose()?;
        for (body_bytes, output_bytes) in [
            (jev_selection, jev::MAX_BYTES),
            (
                codex_diagnosis.unwrap_or(jev_diagnosis),
                if codex_diagnosis.is_some() {
                    codex::MAX_STDOUT + codex::MAX_STDERR
                } else {
                    jev::MAX_BYTES
                },
            ),
            (
                codex_diagnosis.unwrap_or(jev_diagnosis),
                if codex_diagnosis.is_some() {
                    codex::MAX_STDOUT + codex::MAX_STDERR
                } else {
                    jev::MAX_BYTES
                },
            ),
        ] {
            let reserved = body_bytes + 6 * output_bytes + RECEIPT_OVERHEAD;
            require(reserved <= MAX_CALL, "context_evidence_record_budget")?;
            total += reserved;
            largest = largest.max(reserved);
        }
    }
    require(total <= MAX_ARTIFACT, "context_evidence_campaign_budget")?;
    Ok(
        json!({"reserved_call_record_bytes":total,"largest_reserved_call_record_bytes":largest,
        "single_record_limit_bytes":MAX_CALL,"campaign_limit_bytes":MAX_ARTIFACT,
        "reservation":"canonical_request_plus_six_times_bounded_output_plus_32768_receipt_bytes"}),
    )
}

/// The oracle never reaches this function or the provider; it is used only by
/// independent analysis after the recorded responses have been collected.
async fn collect<T: jev::Transport, D: codex::Transport>(
    manifest: &Manifest,
    provider: &mut jev::Jev<T>,
    diagnostic: &mut Option<codex::Diagnostic<D>>,
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
    let (mut reserved_jev, mut reserved_codex) = (0, 0);
    for (index, case) in manifest.cases.iter().enumerate() {
        let mut scores = None;
        for phase in phases(index) {
            if cancel.is_cancelled() {
                report["error"] = "cancelled".into();
                return Ok(calls);
            }
            let (state, questions, _) = stage_request(manifest, case, phase, scores.as_ref())?;
            let coding = diagnostic.is_some() && phase != "selection";
            if manifest.version == 3 && !coding {
                capacity::admit(&state, &questions)?;
            }
            // Reserve and durably checkpoint before a possible dispatch. An
            // interrupted call retains its reservation; there is no resumption.
            require(
                calls.len() < manifest.limits.max_calls as usize,
                "context_call_budget",
            )?;
            let reserved = calls.len() + 1;
            if coding {
                reserved_codex += 1;
            } else {
                reserved_jev += 1;
            }
            require(
                reserved_jev as f64 * per_call_usd() <= manifest.limits.max_reserved_usd
                    && reserved_codex <= 8,
                "context_cost_budget",
            )?;
            report["reserved_calls"] = reserved.into();
            report["reserved_usd"] = json!(reserved_jev as f64 * per_call_usd());
            if manifest.diagnostic.is_some() {
                report["reserved_codex_turns"] = reserved_codex.into();
            }
            report["pending"] = json!({"case":case.id,"phase":phase});
            checkpoint(output, report)?;
            let started = Instant::now();
            let (result, receipt) = if coding {
                let diagnostic = diagnostic.as_mut().expect("configured diagnostic");
                let body = codex::request(diagnostic.profile(), state, &case.diagnoses)?;
                let result = diagnostic.ask(body, cancel).await.map(|()| None);
                (result, diagnostic.take_evidence())
            } else {
                let result = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => Err(Stop::from("cancelled")),
                    result = provider.ask(state, questions) => result.map(Some),
                };
                let mut receipts = provider.take_evidence();
                require(receipts.len() <= 1, "context_receipt_count")?;
                (result, receipts.pop())
            };
            let call = Call {
                case: case.id.clone(),
                phase: phase.into(),
                elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
                error: result.as_ref().err().map(|e| e.0.clone()),
                receipt,
            };
            let mut bytes = encoded(&call)?;
            bytes.push(b'\n');
            require(
                bytes.len()
                    <= if manifest.version == 3 {
                        MAX_CALL
                    } else {
                        LEGACY_MAX_CALL
                    }
                    && used + bytes.len()
                        <= if manifest.version == 3 {
                            MAX_ARTIFACT
                        } else {
                            LEGACY_MAX_ARTIFACT
                        },
                "context_evidence_budget",
            )?;
            events
                .write_all(&bytes)
                .and_then(|_| events.sync_all())
                .map_err(|_| Stop::from("context_evidence_io"))?;
            used += bytes.len();
            calls.push(call);
            report["pending"] = Value::Null;
            report["transport_attempts"] =
                (provider.calls() + diagnostic.as_ref().map_or(0, |d| d.calls())).into();
            checkpoint(output, report)?;
            match result {
                Ok(Some(answers)) if phase == "selection" => scores = Some(answers),
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

struct Observation {
    answers: Option<Answers>,
    choice: Option<String>,
    confidence: Value,
    usage: Option<Value>,
}

fn observation(
    call: &Call,
    expected: &Value,
    questions: &Questions,
    coding: bool,
    serial: usize,
) -> Result<Observation> {
    let mut observed = Observation {
        answers: None,
        choice: None,
        confidence: Value::Null,
        usage: None,
    };
    let Some(receipt) = &call.receipt else {
        require(call.error.is_some(), "context_missing_receipt")?;
        return Ok(observed);
    };
    require(
        receipt["request"] == *expected
            && receipt["request_sha256"] == sha256(&encoded(expected)?)
            && receipt["model"] == expected["model"]
            && receipt["call"] == serial,
        "context_receipt_binding",
    )?;
    if coding {
        require(
            receipt["kind"] == "codex_exec"
                && receipt["reserved_turns"] == 1
                && receipt["deadline_secs"] == codex::DEADLINE_SECS
                && receipt["reserved_usd"].is_null(),
            "context_receipt_binding",
        )?;
    } else {
        require(
            receipt["reserved_input_tokens"] == jev::MAX_TOKENS
                && receipt["reserved_usd"] == json!(per_call_usd())
                && receipt["question_count"] == questions.len(),
            "context_receipt_binding",
        )?;
    }
    if call.error.is_some() {
        require(
            receipt["outcome"] == "failed" || receipt["outcome"] == "interrupted",
            "context_receipt_outcome",
        )?;
        return Ok(observed);
    }
    require(receipt["outcome"] == "accepted", "context_receipt_outcome")?;
    let usage = if coding {
        let transcript = serde_json::from_value(receipt["transcript"].clone())
            .map_err(|_| Stop::from("codex_transcript"))?;
        let (choice, usage) = codex::parse(&transcript, expected)?;
        observed.choice = Some(choice);
        usage
    } else {
        let raw = receipt["response"]
            .as_str()
            .ok_or_else(|| Stop::from("context_response"))?;
        let answers = jev::recorded_answers(raw.as_bytes(), questions)?;
        if let Some(Answer::Choice {
            choice, confidence, ..
        }) = answers.get("diagnosis")
        {
            observed.choice = Some(choice.clone());
            observed.confidence = json!(confidence);
        }
        observed.answers = Some(answers);
        decode(raw.as_bytes())?["usage"].clone()
    };
    require(receipt["usage"] == usage, "context_usage_binding")?;
    observed.usage = Some(usage);
    Ok(observed)
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
    let (mut jev_attempts, mut codex_attempts) = (0, 0);
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
            let coding = manifest.diagnostic.is_some() && phase != "selection";
            let expected = if coding {
                codex::request(
                    manifest.diagnostic.as_ref().expect("configured diagnostic"),
                    state.clone(),
                    &case.diagnoses,
                )?
            } else {
                request(state.clone(), &questions)
            };
            let serial = if coding {
                &mut codex_attempts
            } else {
                &mut jev_attempts
            };
            if call.receipt.is_some() {
                *serial += 1;
            }
            let observed = observation(call, &expected, &questions, coding, *serial)?;
            let input_tokens = observed
                .usage
                .as_ref()
                .and_then(|u| u["input_tokens"].as_u64());
            let mut cost = json!({"case":case.id,"split":case.split,
                "arm":if phase == "baseline" {"baseline"} else {"treatment"},
                "phase":phase,"elapsed_ms":call.elapsed_ms,"input_tokens":input_tokens,
                "estimated_usd":if coding { None } else { input_tokens.map(|n| n as f64*jev::INPUT_USD_PER_MILLION/1e6) },
                "unknown_usage":call.receipt.is_some() && input_tokens.is_none()});
            if manifest.diagnostic.is_some() {
                cost["provider"] = if coding { "codex" } else { "jev" }.into();
                cost["usage"] = observed.usage.clone().unwrap_or(Value::Null);
            }
            costs.push(cost);
            if let Some(error) = &call.error {
                require(position == calls.len(), "context_calls_after_failure")?;
                failures.push(json!({"case":case.id,"phase":phase,"error":error}));
                break;
            }
            if phase == "selection" {
                scores = Some(
                    observed
                        .answers
                        .ok_or_else(|| Stop::from("context_missing_answers"))?,
                );
                selection_ms = call.elapsed_ms;
            } else {
                let choice = observed
                    .choice
                    .ok_or_else(|| Stop::from("context_diagnosis_type"))?;
                let truth = &oracle.cases[&case.id];
                rows.push(json!({"case":case.id,"split":case.split,"arm":phase,
                    "choice":choice,"correct":choice == truth.diagnosis,"confidence":observed.confidence,
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
            let mut item = json!({"split":split,"arm":arm,"planned":planned,"graded":rows.len(),
                "correct":rows.iter().filter(|r| r["correct"] == true).count(),
                "insufficient":rows.iter().filter(|r| r["choice"] == "insufficient").count(),
                "ungraded":planned-rows.len(),"calls":costs.len(),
                "missing_essential":rows.iter().map(|r| r["missing_essential"].as_array().map_or(0,Vec::len)).sum::<usize>(),
                "input_tokens":costs.iter().filter_map(|r| r["input_tokens"].as_u64()).sum::<u64>(),
                "estimated_usd":costs.iter().filter_map(|r| r["estimated_usd"].as_f64()).sum::<f64>(),
                "provider_elapsed_ms":costs.iter().filter_map(|r| r["elapsed_ms"].as_f64()).sum::<f64>(),
                "unknown_usage_calls":costs.iter().filter(|r| r["unknown_usage"] == true).count()});
            if manifest.diagnostic.is_some() {
                item["jev_estimated_usd"] = item["estimated_usd"].clone();
                item["estimated_usd"] = Value::Null;
                item["codex_input_tokens"] = json!(
                    costs
                        .iter()
                        .filter(|c| c["provider"] == "codex")
                        .filter_map(|c| c["input_tokens"].as_u64())
                        .sum::<u64>()
                );
                item["codex_cached_input_tokens"] = json!(
                    costs
                        .iter()
                        .filter(|c| c["provider"] == "codex")
                        .filter_map(|c| c["usage"]["cached_input_tokens"].as_u64())
                        .sum::<u64>()
                );
                item["codex_output_tokens"] = json!(
                    costs
                        .iter()
                        .filter(|c| c["provider"] == "codex")
                        .filter_map(|c| c["usage"]["output_tokens"].as_u64())
                        .sum::<u64>()
                );
            }
            summary.push(item);
        }
    }
    let mut analysis = json!({"rows":rows,"calls":costs,"failures":failures,"summary":summary,
        "transport_attempts":jev_attempts+codex_attempts,"complete":calls.len()==manifest.cases.len()*3 && failures.is_empty(),
        "cache_usage":"not_exposed_by_provider","billed_usd":null});
    if manifest.diagnostic.is_some() {
        analysis["jev_attempts"] = jev_attempts.into();
        analysis["codex_turn_attempts"] = codex_attempts.into();
        analysis["cache_usage"] = "codex_cli_reported_tokens_only_jev_unknown".into();
        analysis["billing"] = "codex_subscription_unknown_jev_estimate_separate".into();
    }
    Ok(analysis)
}

pub async fn campaign<T: jev::Transport>(
    manifest: Manifest,
    oracle: Oracle,
    provider: jev::Jev<T>,
    output: &Path,
    live: bool,
    cancel: &CancellationToken,
) -> Result<Value> {
    run_campaign::<T, codex::Mock>(manifest, oracle, provider, None, output, live, cancel).await
}

pub async fn campaign_with_diagnostic<T: jev::Transport, D: codex::Transport>(
    manifest: Manifest,
    oracle: Oracle,
    provider: jev::Jev<T>,
    diagnostic: codex::Diagnostic<D>,
    output: &Path,
    live: bool,
    cancel: &CancellationToken,
) -> Result<Value> {
    run_campaign(
        manifest,
        oracle,
        provider,
        Some(diagnostic),
        output,
        live,
        cancel,
    )
    .await
}

async fn run_campaign<T: jev::Transport, D: codex::Transport>(
    manifest: Manifest,
    oracle: Oracle,
    mut provider: jev::Jev<T>,
    mut diagnostic: Option<codex::Diagnostic<D>>,
    output: &Path,
    live: bool,
    cancel: &CancellationToken,
) -> Result<Value> {
    require(manifest.version == 3, "context_campaign_requires_v3")?;
    let plan = preflight(&manifest, &oracle)?;
    require(
        provider.calls() == 0 && provider.name() == if live { "jev-live" } else { "jev-mock" },
        "context_provider_mode",
    )?;
    require(
        diagnostic.is_some() == manifest.diagnostic.is_some(),
        "context_diagnostic_mode",
    )?;
    if let Some(diagnostic) = &diagnostic {
        require(
            diagnostic.calls() == 0
                && diagnostic.is_live() == live
                && diagnostic.profile().digest()?
                    == manifest
                        .diagnostic
                        .as_ref()
                        .expect("configured profile")
                        .digest()?,
            "context_diagnostic_identity",
        )?;
    }
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
    let mut report = json!({"version":manifest.version,"mode":if live {"live"} else {"mock"},
        "quality_evidence":live,"model":jev::MODEL,"manifest_sha256":manifest.digest()?,
        "oracle_sha256":sha256(&encoded(&oracle)?),"complete":false,"error":null,
        "reserved_calls":0,"reserved_usd":0.0,"pending":null,"transport_attempts":0});
    if let Some(profile) = &manifest.diagnostic {
        report["coding_model"] = profile.model.clone().into();
        report["diagnostic_profile_sha256"] = profile.digest()?.into();
        report["reserved_codex_turns"] = 0.into();
        report["usd_reservation_scope"] = "jev_only_codex_subscription_billing_unknown".into();
    }
    report["capacity_policy"] = plan["capacity_policy"].clone();
    report["evidence_reservation"] = plan["evidence_reservation"].clone();
    report["capacity_cases"] = plan["cases"].clone();
    checkpoint(output, &report)?;
    let collected = collect(
        &manifest,
        &mut provider,
        &mut diagnostic,
        output,
        cancel,
        &mut report,
    )
    .await;
    match collected {
        Ok(calls) => {
            let analysis = analyze(&manifest, &oracle, &calls)?;
            report["complete"] = analysis["complete"].clone();
            report["analysis"] = analysis;
            report["live_calls"] = if live {
                provider.calls() + diagnostic.as_ref().map_or(0, |d| d.calls())
            } else {
                0
            }
            .into();
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
    let artifact_limit = if manifest.version == 3 {
        MAX_ARTIFACT
    } else {
        LEGACY_MAX_ARTIFACT
    };
    let report = load(&output.join("report.json"), artifact_limit)?;
    if manifest.version == 3 {
        let plan = preflight(&manifest, &oracle)?;
        require(
            report["capacity_policy"] == plan["capacity_policy"]
                && report["evidence_reservation"] == plan["evidence_reservation"]
                && report["capacity_cases"] == plan["cases"],
            "context_replay_capacity",
        )?;
    }
    require(
        report["version"] == manifest.version
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
        .take(artifact_limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Stop::from("context_evidence_io"))?;
    require(
        bytes.len() <= artifact_limit && (bytes.is_empty() || bytes.ends_with(b"\n")),
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
    let reserved_jev = if manifest.diagnostic.is_some() {
        calls.iter().filter(|c| c.phase == "selection").count()
    } else {
        calls.len()
    };
    if let Some(profile) = &manifest.diagnostic {
        require(
            report["coding_model"] == profile.model
                && report["diagnostic_profile_sha256"] == profile.digest()?
                && report["reserved_codex_turns"] == calls.len() - reserved_jev,
            "context_replay_accounting",
        )?;
    }
    require(
        report["quality_evidence"] == live
            && report["reserved_calls"] == calls.len()
            && report["reserved_usd"] == json!(reserved_jev as f64 * per_call_usd())
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
