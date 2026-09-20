//! Admission, freshness and finalization follow the proven Conary/Python loop;
//! no package oracle, game rules, or provider-generated executable operations.
use crate::{
    contract::*,
    evidence::{Evidence, encoded, view_digest},
};
use serde_json::{Value, json};
use std::{
    future::Future,
    path::Path,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

struct Budget<'a> {
    limits: &'a Limits,
    deadline: Instant,
    cancel: &'a CancellationToken,
}
impl Budget<'_> {
    fn admit(&self, attempted: u32, cost: u32) -> Result<()> {
        require(!self.cancel.is_cancelled(), "cancelled")?;
        require(Instant::now() < self.deadline, "time_budget")?;
        require(attempted + cost <= self.limits.inputs, "input_budget")
    }
    async fn call<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        self.admit(0, 0)?;
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs_f64(self.limits.operation_seconds));
        tokio::select! {
            biased;
            _ = self.cancel.cancelled() => Err("cancelled".into()),
            result = tokio::time::timeout(remaining, future) => result.map_err(|_| Stop::from("operation_timeout"))?,
        }
    }
}
fn validate_frame(frame: &Frame) -> Result<()> {
    let o = &frame.observation;
    require(
        o.view.is_object() && encoded(&o.view)?.len() <= 4096,
        "observation_size",
    )?;
    require(encoded(o)?.len() <= 8192, "observation_size")?;
    view_digest(&o.view)?;
    let mut ids = std::collections::BTreeSet::new();
    require(frame.candidates.len() <= 96, "invalid_candidates")?;
    for c in &frame.candidates {
        require(
            !c.id.is_empty()
                && c.id.chars().count() <= 80
                && c.id != "stop"
                && ids.insert(&c.id)
                && c.description.chars().count() <= 512
                && (1..=24).contains(&c.inputs)
                && c.operation.is_object()
                && encoded(&c.operation)?.len() <= 512,
            "invalid_candidates",
        )?;
    }
    require(encoded(&frame.candidates)?.len() <= 16384, "candidate_size")
}
fn checked(check: Verdict) -> Result<Verdict> {
    require(
        check.checks.is_object() && encoded(&check)?.len() <= 1024,
        "invalid_evaluation",
    )?;
    Ok(check)
}
fn receipts(provider: &mut Option<&mut dyn Provider>, evidence: &mut Evidence) -> Result<()> {
    if let Some(p) = provider.as_deref_mut() {
        let rows = p.take_evidence();
        require(
            rows.len() <= 1 && encoded(&rows)?.len() <= 131072,
            "provider_evidence_size",
        )?;
        for row in rows {
            evidence.event("provider_receipt", &row)?;
        }
    }
    Ok(())
}
fn code(error: Stop) -> String {
    if !error.0.is_empty()
        && error.0.len() <= 80
        && error
            .0
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        error.0
    } else {
        "adapter_error".into()
    }
}

/// One controller; replay never invokes a provider. Final evaluation and close
/// have independent time reserves and survive cancellation and every refusal.
pub async fn run(
    adapter: &mut dyn Adapter,
    mut provider: Option<&mut dyn Provider>,
    replay: Option<Value>,
    output: &Path,
    limits: Limits,
    cancel: &CancellationToken,
) -> Result<Value> {
    limits.validate()?;
    require(
        provider.is_some() != replay.is_some(),
        "provide_exactly_one_selector_or_replay",
    )?;
    let mut evidence = Evidence::create(output, limits.evidence_bytes)?;
    let started = Instant::now();
    let budget = Budget {
        limits: &limits,
        deadline: started + Duration::from_secs_f64(limits.seconds - 2. * limits.final_seconds),
        cancel,
    };
    let mut report = json!({"version":1,"controller":"rust-v1","identity":null,"limits":limits,
        "provider":provider.as_ref().map(|p|p.name()).unwrap_or("none-replay"),
        "requests":0,"attempted_inputs":0,"operations":[],"evaluations":[],"stop":"not_started",
        "setup_mode":null,"setup_verified":false,"reset_verified":false,"final":null,"cleanup":false});
    let mut records = vec![];
    let mut operations = vec![];
    let mut evaluations = vec![];
    let mut attempted = 0;
    let mut requests = 0;
    let mut descriptor = None;
    let mut setup_verified = false;
    let outcome: Result<&str> = async {
        let initial = budget.call(adapter.describe()).await?;
        require(initial.identity.is_object() && encoded(&initial.identity)?.len() <= 4096, "identity_size")?;
        require(["reset", "attach"].contains(&initial.setup_mode.as_str()), "invalid_setup_mode")?;
        report["identity"] = initial.identity.clone();
        report["setup_mode"] = initial.setup_mode.clone().into();
        descriptor = Some(initial.clone());
        let saved: Option<Replay> = if let Some(raw) = &replay {
            require(initial.setup_mode == "reset", "replay_unavailable")?;
            require(encoded(raw)?.len() <= 65536, "replay_identity_or_shape")?;
            let saved: Replay = serde_json::from_value(raw.clone()).map_err(|_| Stop::from("replay_identity_or_shape"))?;
            require(saved.version == 1 && saved.complete && saved.identity == initial.identity
                && saved.steps.len() <= limits.inputs as usize, "replay_identity_or_shape")?;
            Some(saved)
        } else { None };
        evidence.event("started", &json!({"identity":initial.identity,"limits":limits,"setup_mode":initial.setup_mode}))?;
        budget.call(adapter.reset()).await?;
        let reset = checked(budget.call(adapter.evaluate("reset", None)).await?)?;
        evidence.event("reset_checks", &reset)?;
        require(reset.ok, "reset_unverified")?;
        setup_verified = true;
        let baseline = budget.call(adapter.observe()).await?;
        validate_frame(&baseline)?;
        let mut no_progress = 0;
        loop {
            budget.admit(attempted, 0)?;
            let frame = budget.call(adapter.observe()).await?;
            validate_frame(&frame)?;
            let observation = &frame.observation;
            require(observation.environment == baseline.observation.environment
                && observation.epoch == baseline.observation.epoch, "environment_changed")?;
            if let Some(terminal) = &observation.terminal { return Err(Stop(terminal.clone())); }
            let saved_step = if let Some(saved) = &saved {
                if records.len() == saved.steps.len() { return Ok("replay_complete"); }
                Some(&saved.steps[records.len()])
            } else { None };
            let chosen = if let Some(step) = saved_step {
                require(step.view == view_digest(&observation.view)?, "replay_precondition")?;
                let matches: Vec<_> = frame.candidates.iter().filter(|c|c.operation == step.operation).collect();
                require(matches.len() == 1, "replay_candidate_unavailable")?;
                matches[0].id.clone()
            } else {
                require(requests < limits.requests, "request_budget")?;
                let mut candidates: serde_json::Map<String, Value> = frame.candidates.iter()
                    .map(|c|(c.id.clone(), c.description.clone().into())).collect();
                candidates.insert("stop".into(), "Stop the experiment.".into());
                let request = json!({"version":1,"observation":observation.view,"candidates":candidates,
                    "remaining_inputs":limits.inputs-attempted});
                let size = encoded(&request)?.len();
                require(size <= 16384, "observation_size")?;
                let p = provider.as_deref_mut().ok_or_else(|| Stop::from("missing_provider"))?;
                require(p.evidence_limit() <= 131072, "provider_evidence_size")?;
                evidence.reserve(p.evidence_limit() + size + 2048)?;
                evidence.event("request", &request)?;
                requests += 1;
                let answer = budget.call(p.select(request)).await;
                receipts(&mut provider, &mut evidence)?;
                answer?
            };
            budget.admit(attempted, 0)?;
            require(chosen.chars().count() <= 80, "invalid_decision")?;
            evidence.event("decision", &json!({"candidate_id":chosen}))?;
            if chosen == "stop" { return Ok("selector_stop"); }
            let candidate = frame.candidates.iter().find(|c|c.id == chosen).ok_or_else(|| Stop::from("unknown_candidate"))?;
            let current_identity = budget.call(adapter.verify()).await?;
            require(current_identity == initial, "identity_changed")?;
            let current = budget.call(adapter.observe()).await?;
            validate_frame(&current)?;
            require(current.observation == *observation, "stale_observation")?;
            require(current.candidates == frame.candidates, "candidate_changed")?;
            require(!candidate.needs_ready || current.observation.ready, "busy_refused")?;
            budget.admit(attempted, candidate.inputs)?;
            evidence.event("intent", &json!({"operation":candidate.operation,"view":observation.view}))?;
            budget.admit(attempted, candidate.inputs)?;
            // Accounting precedes the uncertain side effect; never drop an attempt.
            attempted += candidate.inputs;
            operations.push(candidate.operation.clone());
            let receipt = budget.call(adapter.execute(&candidate.operation)).await?;
            require(encoded(&receipt)?.len() <= 4096, "receipt_size")?;
            evidence.event("receipt", &receipt)?;
            let check = checked(budget.call(adapter.evaluate("after", Some(&candidate.operation))).await?)?;
            evidence.event("checked", &check)?;
            evaluations.push(check.clone());
            records.push(Step { operation: candidate.operation.clone(), view: view_digest(&observation.view)?, verdict: check.clone() });
            if let Some(step) = saved_step { require(step.verdict == check, "replay_mismatch")?; }
            require(check.ok, "evaluation_failed")?;
            no_progress = if check.progress { 0 } else { no_progress + 1 };
            require(no_progress < limits.no_progress, "no_progress")?;
        }
    }.await;
    report["stop"] = match outcome {
        Ok(reason) => reason.into(),
        Err(error) => code(error).into(),
    };
    let final_timeout = Duration::from_secs_f64(limits.final_seconds);
    let finalization = async {
        let check = checked(adapter.evaluate("final", None).await?)?;
        let identity = adapter.describe().await?;
        Ok::<_, Stop>((check, identity))
    };
    let (final_check, identity_stable) =
        match tokio::time::timeout(final_timeout, finalization).await {
            Ok(Ok((check, identity))) => (Some(check), descriptor.as_ref() == Some(&identity)),
            _ => (None, false),
        };
    if descriptor.is_some() && final_check.is_some() && !identity_stable {
        report["stop"] = "identity_changed".into();
    }
    report["identity_stable"] = identity_stable.into();
    report["final"] = final_check
        .as_ref()
        .map(|v| json!(v))
        .unwrap_or_else(|| json!({"ok":false,"error_type":"final_check_failed"}));
    let final_identity = descriptor.as_ref().is_some_and(|d| d.setup_mode == "reset");
    // The close contract includes joining the owned process; forced kill is a failure.
    let cleanup = matches!(
        tokio::time::timeout(final_timeout, adapter.close()).await,
        Ok(Ok(()))
    );
    // A cancelled process-provider response may have drained during finalization.
    if let Err(error) = receipts(&mut provider, &mut evidence) {
        report["stop"] = code(error).into();
    }
    let final_ok = final_check.as_ref().is_some_and(|v| v.ok);
    let replayable = setup_verified
        && final_identity
        && identity_stable
        && records.len() == operations.len()
        && final_ok
        && cleanup
        && report["stop"] != "identity_changed";
    report["requests"] = requests.into();
    report["attempted_inputs"] = attempted.into();
    report["operations"] = json!(operations);
    report["evaluations"] = json!(evaluations);
    report["setup_verified"] = setup_verified.into();
    report["reset_verified"] = (setup_verified && final_identity).into();
    report["cleanup"] = cleanup.into();
    report["elapsed_seconds"] = started.elapsed().as_secs_f64().into();
    report["replayable"] = replayable.into();
    report["replay_complete"] = (replay.is_some()
        && report["stop"] == "replay_complete"
        && final_ok
        && cleanup
        && identity_stable)
        .into();
    let saved = Replay {
        version: 1,
        identity: report["identity"].clone(),
        complete: replayable,
        steps: records,
    };
    evidence.finish(&report, &saved)?;
    Ok(report)
}
