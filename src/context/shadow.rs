//! Bounded scoring-only observations over verified source packets.
//!
//! Scores are advisory measurements. They do not select context, establish
//! coding correctness, or grant authority to act on source text.

use super::{
    Case as ContextCase, capacity, lexical, packet, per_call_usd, request, scoring_questions,
};
use crate::{
    CancellationToken, Provider, Result, Stop,
    comparison::Split,
    decision::{Answer, Answers, Questions},
    evidence::{decode, encoded, load, sha256},
    jev, require,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::Instant,
};

pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_PREFLIGHT_BYTES: usize = 1024 * 1024;
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const MAX_CALL_BYTES: usize = 1024 * 1024;
const MAX_CALL_LOG_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const RECEIPT_OVERHEAD: usize = 32 * 1024;
const MAX_CALLS: usize = 4;
const MAX_RESERVED_USD: f64 = 0.02;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_calls: u32,
    pub max_reserved_usd: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub packet: packet::Packet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub limits: Limits,
    pub cases: Vec<Case>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rank {
    id: String,
    rank: usize,
    score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    case: String,
    elapsed_ms: f64,
    error: Option<String>,
    receipt: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    version: u32,
    mode: String,
    model: String,
    manifest_sha256: String,
    preflight_sha256: String,
    complete: bool,
    error: Option<String>,
    pending: Option<String>,
    reserved_calls: usize,
    reserved_usd: f64,
    transport_attempts: u32,
    live_calls: u32,
    billed_usd: Option<f64>,
    analysis: Value,
    wall_elapsed_ms: f64,
}

struct PreparedCase {
    id: String,
    packet_sha256: String,
    state: Value,
    questions: Questions,
    request: Value,
    bm25_ranking: Vec<Rank>,
    record_reservation: usize,
}

struct Prepared {
    plan: Value,
    cases: Vec<PreparedCase>,
    manifest_sha256: String,
    preflight_sha256: String,
    call_log_reservation: usize,
}

impl Manifest {
    pub fn validate(&self) -> Result<()> {
        require(
            self.version == 1
                && (1..=MAX_CALLS).contains(&self.cases.len())
                && self.limits.max_calls >= self.cases.len() as u32
                && self.limits.max_calls <= MAX_CALLS as u32
                && self.limits.max_reserved_usd.is_finite()
                && self.limits.max_reserved_usd > 0.0
                && self.limits.max_reserved_usd <= MAX_RESERVED_USD
                && self.cases.len() as f64 * per_call_usd() <= self.limits.max_reserved_usd,
            "shadow_limits",
        )?;
        let mut ids = BTreeSet::new();
        let revision = &self.cases[0].packet.source_revision;
        for case in &self.cases {
            require(
                super::id(&case.id)
                    && ids.insert(&case.id)
                    && !case.packet.required.is_empty()
                    && (2..=8).contains(&case.packet.chunks.len())
                    && case.packet.source_revision == *revision,
                "shadow_case",
            )?;
            case.packet.validate()?;
        }
        require(
            encoded(self)?.len() <= MAX_MANIFEST_BYTES,
            "shadow_manifest_size",
        )
    }

    pub fn digest(&self) -> Result<String> {
        self.validate()?;
        Ok(sha256(&encoded(self)?))
    }

    pub fn provider_config(&self) -> jev::Config {
        jev::Config {
            request_limit: self.limits.max_calls,
            request_bytes: jev::MAX_REQUEST_BYTES,
            ..jev::Config::default()
        }
    }
}

pub fn read_manifest(path: &Path) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_value(load(path, MAX_MANIFEST_BYTES)?)
        .map_err(|_| Stop::from("shadow_manifest_schema"))?;
    manifest.validate()?;
    Ok(manifest)
}

fn context_case(case: &Case) -> ContextCase {
    let (required, chunks) = case.packet.context_chunks();
    let baseline_order = chunks.iter().map(|chunk| chunk.id.clone()).collect();
    ContextCase {
        id: case.id.clone(),
        split: Split::Calibration,
        task: case.packet.task.clone(),
        mandatory: required,
        chunks,
        baseline_order,
        diagnoses: BTreeMap::new(),
    }
}

fn ranking(rows: Vec<(String, f64)>) -> Vec<Rank> {
    rows.into_iter()
        .enumerate()
        .map(|(index, (id, score))| Rank {
            id,
            rank: index + 1,
            score,
        })
        .collect()
}

fn prepare(manifest: &Manifest, repo: Option<&Path>) -> Result<Prepared> {
    manifest.validate()?;
    manifest.provider_config().validate()?;
    let manifest_sha256 = manifest.digest()?;
    let mut cases = Vec::new();
    let mut plan_cases = Vec::new();
    let mut call_log_reservation = 0usize;
    for (index, source_case) in manifest.cases.iter().enumerate() {
        if let Some(repo) = repo {
            packet::verify(repo, &source_case.packet)?;
        }
        let packet_sha256 = source_case.packet.digest()?;
        let case = context_case(source_case);
        let questions = scoring_questions(&case);
        require(
            questions.len() == case.chunks.len() && encoded(&questions)?.len() <= 8192,
            "shadow_question_budget",
        )?;
        for question in questions.values() {
            question.validate()?;
        }
        let bm25_ranking = ranking(lexical::ranking(&case));
        let state = json!({"task":case.task,"mandatory":case.mandatory,"evidence":case.chunks});
        let body = request(state.clone(), &questions);
        let body_bytes = encoded(&body)?;
        require(
            body_bytes.len() <= jev::MAX_REQUEST_BYTES,
            "shadow_request_budget",
        )?;
        let jev_capacity = capacity::admit(&state, &questions)?;
        let record_reservation = body_bytes.len() + 6 * jev::MAX_BYTES + RECEIPT_OVERHEAD;
        require(record_reservation <= MAX_CALL_BYTES, "shadow_record_budget")?;
        call_log_reservation += record_reservation;
        require(
            call_log_reservation <= MAX_CALL_LOG_BYTES,
            "shadow_log_budget",
        )?;
        let request_sha256 = sha256(&body_bytes);
        plan_cases.push(json!({
            "id":source_case.id,
            "call":index+1,
            "packet_sha256":packet_sha256,
            "request_sha256":request_sha256,
            "request_bytes":body_bytes.len(),
            "required_count":source_case.packet.required.len(),
            "candidate_count":source_case.packet.chunks.len(),
            "bm25_ranking":bm25_ranking,
            "jev_capacity":jev_capacity,
            "source_verification":"passed_at_preflight"
        }));
        cases.push(PreparedCase {
            id: source_case.id.clone(),
            packet_sha256,
            state,
            questions,
            request: body,
            bm25_ranking,
            record_reservation,
        });
    }
    let manifest_bytes = encoded(manifest)?.len();
    let total_reservation =
        manifest_bytes + MAX_PREFLIGHT_BYTES + MAX_REPORT_BYTES + call_log_reservation;
    require(
        total_reservation <= MAX_OUTPUT_BYTES,
        "shadow_output_budget",
    )?;
    let plan = json!({
        "version":1,
        "manifest_sha256":manifest_sha256,
        "model":jev::MODEL,
        "source_revision":manifest.cases[0].packet.source_revision,
        "planned_calls":manifest.cases.len(),
        "reserved_usd":manifest.cases.len() as f64*per_call_usd(),
        "capacity_policy":capacity::policy(),
        "evidence_reservation":{
            "manifest_bytes":manifest_bytes,
            "preflight_limit_bytes":MAX_PREFLIGHT_BYTES,
            "report_limit_bytes":MAX_REPORT_BYTES,
            "call_log_reserved_bytes":call_log_reservation,
            "call_record_limit_bytes":MAX_CALL_BYTES,
            "total_reserved_bytes":total_reservation,
            "output_limit_bytes":MAX_OUTPUT_BYTES
        },
        "cases":plan_cases,
    });
    require(
        encoded(&plan)?.len() <= MAX_PREFLIGHT_BYTES,
        "shadow_preflight_size",
    )?;
    let preflight_sha256 = sha256(&encoded(&plan)?);
    Ok(Prepared {
        plan,
        cases,
        manifest_sha256,
        preflight_sha256,
        call_log_reservation,
    })
}

/// Verify every embedded packet and admit all planned requests before output.
pub fn preflight(manifest: &Manifest, repo: &Path) -> Result<Value> {
    Ok(prepare(manifest, Some(repo))?.plan)
}

fn write_new(path: &Path, value: &impl Serialize, limit: usize) -> Result<()> {
    let bytes = encoded(value)?;
    require(bytes.len() <= limit, "shadow_artifact_size")?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| {
            file.write_all(&bytes)?;
            file.sync_all()
        })
        .map_err(|_| Stop::from("shadow_evidence_io"))
}

fn checkpoint(output: &Path, report: &Report) -> Result<()> {
    let pending = output.join("report.pending.json");
    write_new(&pending, report, MAX_REPORT_BYTES)?;
    fs::rename(pending, output.join("report.json")).map_err(|_| Stop::from("shadow_evidence_io"))
}

fn jev_ranking(prepared: &PreparedCase, answers: &Answers) -> Result<Vec<Rank>> {
    require(
        answers.keys().eq(prepared.questions.keys()),
        "shadow_score_ids",
    )?;
    let mut scored = Vec::new();
    for row in &prepared.bm25_ranking {
        let answer = answers
            .get(&row.id)
            .ok_or_else(|| Stop::from("shadow_score_ids"))?;
        answer.validate_for(&prepared.questions[&row.id])?;
        let Answer::Score { score, .. } = answer else {
            return Err("shadow_score_type".into());
        };
        scored.push((row.id.clone(), *score));
    }
    // A stable sort preserves the full BM25 order when Jev scores tie.
    scored.sort_by(|a, b| {
        if a.1 == b.1 {
            std::cmp::Ordering::Equal
        } else {
            b.1.total_cmp(&a.1)
        }
    });
    Ok(ranking(scored))
}

fn observe(
    call: &Call,
    prepared: &PreparedCase,
    serial: usize,
) -> Result<(Option<Answers>, Option<Value>)> {
    let Some(receipt) = &call.receipt else {
        require(
            call.error.as_deref().is_some_and(|error| {
                matches!(
                    error,
                    "cancelled"
                        | "provider_request_budget"
                        | "judgment_action_config"
                        | "provider_request_size"
                )
            }),
            "shadow_missing_receipt",
        )?;
        return Ok((None, None));
    };
    require(
        receipt["version"] == 1
            && receipt["model"] == jev::MODEL
            && receipt["call"] == serial
            && receipt["request"] == prepared.request
            && receipt["request_sha256"] == sha256(&encoded(&prepared.request)?)
            && receipt["question_count"] == prepared.questions.len()
            && receipt["reserved_input_tokens"] == jev::MAX_TOKENS
            && receipt["reserved_usd"] == json!(per_call_usd())
            && receipt["billed_usd"].is_null(),
        "shadow_receipt_binding",
    )?;
    if let Some(error) = &call.error {
        require(
            (receipt["outcome"] == "failed" && receipt["error"] == *error)
                || (receipt["outcome"] == "interrupted" && error == "cancelled"),
            "shadow_receipt_outcome",
        )?;
        return Ok((None, None));
    }
    require(
        receipt["outcome"] == "accepted"
            && receipt["http_status"] == 200
            && receipt["error"].is_null(),
        "shadow_receipt_outcome",
    )?;
    let raw = receipt["response"]
        .as_str()
        .ok_or_else(|| Stop::from("shadow_response"))?;
    let answers = jev::recorded_answers(raw.as_bytes(), &prepared.questions)?;
    let usage = decode(raw.as_bytes())?["usage"].clone();
    let input_tokens = usage["input_tokens"]
        .as_u64()
        .ok_or_else(|| Stop::from("shadow_usage"))?;
    require(
        receipt["usage"] == usage
            && receipt["estimated_usd"]
                == json!(input_tokens as f64 * jev::INPUT_USD_PER_MILLION / 1e6),
        "shadow_usage_binding",
    )?;
    Ok((Some(answers), Some(usage)))
}

fn analyze(prepared: &Prepared, calls: &[Call]) -> Result<Value> {
    require(calls.len() <= prepared.cases.len(), "shadow_call_count")?;
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let mut receipt_count = 0;
    let mut unknown_usage_calls = 0;
    for (index, call) in calls.iter().enumerate() {
        let expected = &prepared.cases[index];
        require(
            call.case == expected.id && call.elapsed_ms.is_finite() && call.elapsed_ms >= 0.0,
            "shadow_call_order",
        )?;
        if call.receipt.is_some() {
            receipt_count += 1;
        }
        let (answers, usage) = observe(call, expected, receipt_count)?;
        let ranked = answers
            .as_ref()
            .map(|answers| jev_ranking(expected, answers))
            .transpose()?;
        let input_tokens = usage
            .as_ref()
            .and_then(|value| value["input_tokens"].as_u64());
        let estimated_usd =
            input_tokens.map(|tokens| tokens as f64 * jev::INPUT_USD_PER_MILLION / 1e6);
        if call.receipt.is_some() && usage.is_none() {
            unknown_usage_calls += 1;
        }
        if let Some(error) = &call.error {
            require(index + 1 == calls.len(), "shadow_calls_after_failure")?;
            failures.push(json!({"case":call.case,"error":error}));
        }
        rows.push(json!({
            "id":call.case,
            "packet_sha256":expected.packet_sha256,
            "request_sha256":sha256(&encoded(&expected.request)?),
            "bm25_ranking":expected.bm25_ranking,
            "jev_ranking":ranked,
            "elapsed_ms":call.elapsed_ms,
            "usage":usage,
            "estimated_usd":estimated_usd,
            "error":call.error
        }));
    }
    Ok(json!({
        "complete":calls.len() == prepared.cases.len() && failures.is_empty(),
        "cases":rows,
        "failures":failures,
        "unknown_usage_calls":unknown_usage_calls
    }))
}

/// Run one score batch per case. Failure and cancellation end the prefix.
pub async fn campaign<T: jev::Transport>(
    manifest: Manifest,
    repo: &Path,
    mut provider: jev::Jev<T>,
    output: &Path,
    live: bool,
    cancel: &CancellationToken,
) -> Result<Value> {
    let prepared = prepare(&manifest, Some(repo))?;
    require(
        provider.calls() == 0 && provider.name() == if live { "jev-live" } else { "jev-mock" },
        "shadow_provider_mode",
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
    write_new(&output.join("manifest.json"), &manifest, MAX_MANIFEST_BYTES)?;
    write_new(
        &output.join("preflight.json"),
        &prepared.plan,
        MAX_PREFLIGHT_BYTES,
    )?;
    let mut events = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("calls.jsonl"))
        .map_err(|_| Stop::from("shadow_evidence_io"))?;
    let started = Instant::now();
    let mut calls = Vec::new();
    let mut used = 0usize;
    let mut report = Report {
        version: 1,
        mode: if live { "live" } else { "mock" }.into(),
        model: jev::MODEL.into(),
        manifest_sha256: prepared.manifest_sha256.clone(),
        preflight_sha256: prepared.preflight_sha256.clone(),
        complete: false,
        error: None,
        pending: None,
        reserved_calls: 0,
        reserved_usd: 0.0,
        transport_attempts: 0,
        live_calls: 0,
        billed_usd: None,
        analysis: analyze(&prepared, &[])?,
        wall_elapsed_ms: 0.0,
    };
    checkpoint(output, &report)?;
    for (index, source_case) in manifest.cases.iter().enumerate() {
        if cancel.is_cancelled() {
            report.error = Some("cancelled".into());
            break;
        }
        if let Err(error) = packet::verify(repo, &source_case.packet) {
            report.error = Some(error.0);
            break;
        }
        if cancel.is_cancelled() {
            report.error = Some("cancelled".into());
            break;
        }
        let expected = &prepared.cases[index];
        report.reserved_calls = index + 1;
        report.reserved_usd = report.reserved_calls as f64 * per_call_usd();
        report.pending = Some(source_case.id.clone());
        checkpoint(output, &report)?;
        let call_started = Instant::now();
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(Stop::from("cancelled")),
            result = provider.ask(expected.state.clone(), expected.questions.clone()) => result,
        };
        let mut receipts = provider.take_evidence();
        require(receipts.len() <= 1, "shadow_receipt_count")?;
        let call = Call {
            case: source_case.id.clone(),
            elapsed_ms: call_started.elapsed().as_secs_f64() * 1000.0,
            error: result.as_ref().err().map(|error| error.0.clone()),
            receipt: receipts.pop(),
        };
        let mut bytes = encoded(&call)?;
        bytes.push(b'\n');
        require(
            bytes.len() <= expected.record_reservation
                && bytes.len() <= MAX_CALL_BYTES
                && used + bytes.len() <= prepared.call_log_reservation
                && used + bytes.len() <= MAX_CALL_LOG_BYTES,
            "shadow_evidence_budget",
        )?;
        events
            .write_all(&bytes)
            .and_then(|_| events.sync_all())
            .map_err(|_| Stop::from("shadow_evidence_io"))?;
        used += bytes.len();
        calls.push(call);
        report.pending = None;
        report.transport_attempts = provider.calls();
        report.live_calls = if live { provider.calls() } else { 0 };
        report.analysis = analyze(&prepared, &calls)?;
        report.error = result.err().map(|error| error.0);
        report.wall_elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        checkpoint(output, &report)?;
        if report.error.is_some() {
            break;
        }
    }
    report.analysis = analyze(&prepared, &calls)?;
    report.complete = report.analysis["complete"] == true;
    report.wall_elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    checkpoint(output, &report)?;
    serde_json::to_value(report).map_err(|_| Stop::from("shadow_report_json"))
}

/// Check stored packet/request/response consistency without current-source claims.
pub fn replay(output: &Path) -> Result<Value> {
    let manifest = read_manifest(&output.join("manifest.json"))?;
    let prepared = prepare(&manifest, None)?;
    require(
        load(&output.join("preflight.json"), MAX_PREFLIGHT_BYTES)? == prepared.plan,
        "shadow_replay_preflight",
    )?;
    let report: Report =
        serde_json::from_value(load(&output.join("report.json"), MAX_REPORT_BYTES)?)
            .map_err(|_| Stop::from("shadow_replay_report"))?;
    require(
        report.version == 1
            && (report.mode == "mock" || report.mode == "live")
            && report.model == jev::MODEL
            && report.manifest_sha256 == prepared.manifest_sha256
            && report.preflight_sha256 == prepared.preflight_sha256
            && report.pending.is_none()
            && report.billed_usd.is_none()
            && report.wall_elapsed_ms.is_finite()
            && report.wall_elapsed_ms >= 0.0,
        "shadow_replay_identity",
    )?;
    let mut bytes = Vec::new();
    File::open(output.join("calls.jsonl"))
        .map_err(|_| Stop::from("shadow_evidence_io"))?
        .take(MAX_CALL_LOG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Stop::from("shadow_evidence_io"))?;
    require(
        bytes.len() <= prepared.call_log_reservation
            && bytes.len() <= MAX_CALL_LOG_BYTES
            && (bytes.is_empty() || bytes.ends_with(b"\n")),
        "shadow_replay_log_size",
    )?;
    let mut calls = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        require(calls.len() < prepared.cases.len(), "shadow_replay_calls")?;
        require(
            line.len() < prepared.cases[calls.len()].record_reservation,
            "shadow_replay_record_size",
        )?;
        let call: Call =
            serde_json::from_value(decode(line)?).map_err(|_| Stop::from("shadow_replay_call"))?;
        calls.push(call);
    }
    let analysis = analyze(&prepared, &calls)?;
    let receipts = calls.iter().filter(|call| call.receipt.is_some()).count() as u32;
    require(
        report.analysis == analysis
            && report.complete == (analysis["complete"] == true)
            && report.reserved_calls == calls.len()
            && report.reserved_usd == calls.len() as f64 * per_call_usd()
            && report.transport_attempts == receipts
            && report.live_calls == if report.mode == "live" { receipts } else { 0 },
        "shadow_replay_accounting",
    )?;
    if report.complete {
        require(report.error.is_none(), "shadow_replay_terminal")?;
    } else if let Some(last) = calls.last().and_then(|call| call.error.as_ref()) {
        require(
            report.error.as_ref() == Some(last),
            "shadow_replay_terminal",
        )?;
    } else {
        require(
            calls.len() < prepared.cases.len()
                && report
                    .error
                    .as_deref()
                    .is_some_and(|error| error == "cancelled" || error.starts_with("packet_")),
            "shadow_replay_terminal",
        )?;
    }
    Ok(json!({
        "verified":true,
        "provider_calls":0,
        "source_freshness":"not_checked_by_replay",
        "complete":report.complete,
        "analysis":analysis
    }))
}
