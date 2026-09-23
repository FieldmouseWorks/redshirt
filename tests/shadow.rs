use async_trait::async_trait;
use redshirt::{
    CancellationToken, Result, Stop,
    context::{packet, shadow},
    decision::{Question, Questions},
    evidence::{decode, encoded, sha256},
    jev,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::{Command, Output},
    sync::{Arc, Mutex},
};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn fixture() -> (TempDir, shadow::Manifest) {
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q"]);
    git(
        repo.path(),
        &["config", "user.email", "shadow@example.invalid"],
    );
    git(repo.path(), &["config", "user.name", "Shadow Test"]);
    fs::create_dir(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("src/evidence.txt"),
        b"MANDATORY: read only.\nalpha parser trace is direct evidence.\nbeta renderer path is background.\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "source"]);
    let revision = git(repo.path(), &["rev-parse", "HEAD"]);
    let reference = |id: &str, line| packet::SourceRef {
        id: id.into(),
        path: "src/evidence.txt".into(),
        start_line: line,
        end_line: line,
    };
    let packet = packet::build(
        repo.path(),
        &packet::Spec {
            version: 1,
            source_revision: revision,
            task: "Trace the alpha parser path.".into(),
            required: vec![reference("policy", 1)],
            chunks: vec![reference("alpha", 2), reference("beta", 3)],
        },
    )
    .unwrap();
    let manifest = shadow::Manifest {
        version: 1,
        limits: shadow::Limits {
            max_calls: 2,
            max_reserved_usd: 0.01,
        },
        cases: vec![shadow::Case {
            id: "trace".into(),
            packet,
        }],
    };
    (repo, manifest)
}

fn two_cases(manifest: &mut shadow::Manifest) {
    manifest.cases.push(shadow::Case {
        id: "followup".into(),
        packet: manifest.cases[0].packet.clone(),
    });
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_redshirt-shadow"))
        .args(args)
        .output()
        .unwrap()
}

fn write_manifest(repo: &Path, manifest: &shadow::Manifest) -> std::path::PathBuf {
    let path = repo.join("shadow-manifest.json");
    fs::write(&path, encoded(manifest).unwrap()).unwrap();
    path
}

#[derive(Clone, Copy)]
enum Behavior {
    Ok,
    Fail,
    Hang,
}

struct Probe {
    seen: Arc<Mutex<Vec<Value>>>,
    scores: BTreeMap<String, f64>,
    behavior: Behavior,
    started: Option<Arc<tokio::sync::Notify>>,
}

#[async_trait]
impl jev::Transport for Probe {
    async fn post(&mut self, body: Vec<u8>) -> Result<(u16, Vec<u8>)> {
        let body = decode(&body)?;
        self.seen.lock().unwrap().push(body.clone());
        if let Some(started) = &self.started {
            started.notify_one();
        }
        match self.behavior {
            Behavior::Fail => return Err(Stop::from("synthetic_transport_error")),
            Behavior::Hang => return std::future::pending().await,
            Behavior::Ok => {}
        }
        let questions: Questions = serde_json::from_value(body["questions"].clone()).unwrap();
        let answers: BTreeMap<_, _> = questions
            .into_iter()
            .map(|(id, question)| {
                let Question::Score { criteria, .. } = question else {
                    panic!("shadow request contains a non-score question");
                };
                let score = self.scores.get(&id).copied().unwrap_or(0.0);
                let legend: BTreeMap<_, _> = criteria
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| (index.to_string(), name))
                    .collect();
                let probabilities = BTreeMap::from([(format!("{score:.0}"), 1.0)]);
                (
                    id,
                    json!({"type":"score","score":score,"legend":legend,
                        "probabilities":probabilities,"confidence":1.0}),
                )
            })
            .collect();
        Ok((
            200,
            encoded(&json!({"model":jev::MODEL,"answers":answers,
                "usage":{"input_tokens":321,"output_tokens":12}}))?,
        ))
    }
}

fn probe(behavior: Behavior, scores: &[(&str, f64)]) -> (Probe, Arc<Mutex<Vec<Value>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    (
        Probe {
            seen: seen.clone(),
            scores: scores
                .iter()
                .map(|(id, score)| ((*id).into(), *score))
                .collect(),
            behavior,
            started: None,
        },
        seen,
    )
}

#[test]
fn cli_preflight_mock_campaign_and_replay_keep_full_evidence() {
    let (repo, mut manifest) = fixture();
    two_cases(&mut manifest);
    let manifest_path = write_manifest(repo.path(), &manifest);
    let output = repo.path().join("run");
    let args = [
        "--manifest",
        manifest_path.to_str().unwrap(),
        "--repo",
        repo.path().to_str().unwrap(),
    ];
    let preflight = cli(&[args[0], args[1], args[2], args[3], "--preflight"]);
    assert!(
        preflight.status.success(),
        "{}",
        String::from_utf8_lossy(&preflight.stderr)
    );
    let plan: Value = serde_json::from_slice(&preflight.stdout).unwrap();
    assert_eq!(plan["planned_calls"], 2);
    assert_eq!(
        plan["cases"][0]["bm25_ranking"].as_array().unwrap().len(),
        2
    );
    assert_eq!(plan["cases"][0]["bm25_ranking"][0]["rank"], 1);
    assert_eq!(
        plan["cases"][0]["source_verification"],
        "passed_at_preflight"
    );
    assert!(!output.exists());
    let run = cli(&[
        args[0],
        args[1],
        args[2],
        args[3],
        "--output",
        output.to_str().unwrap(),
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let report: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(report["reserved_calls"], 2);
    assert_eq!(report["transport_attempts"], 2);
    assert_eq!(report["analysis"]["cases"].as_array().unwrap().len(), 2);
    assert_eq!(
        report["analysis"]["cases"][0]["jev_ranking"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(report["billed_usd"].is_null());
    assert!(report.get("oracle").is_none());
    for filename in [
        "manifest.json",
        "preflight.json",
        "report.json",
        "calls.jsonl",
    ] {
        assert!(output.join(filename).exists(), "missing {filename}");
    }
    let saved_plan: Value = decode(&fs::read(output.join("preflight.json")).unwrap()).unwrap();
    assert_eq!(saved_plan, plan);
    let calls = fs::read_to_string(output.join("calls.jsonl")).unwrap();
    let first: Value = serde_json::from_str(calls.lines().next().unwrap()).unwrap();
    let state = &first["receipt"]["request"]["state"];
    assert_eq!(state["mandatory"].as_array().unwrap().len(), 1);
    assert_eq!(state["evidence"].as_array().unwrap().len(), 2);
    assert_eq!(state["mandatory"][0]["text"], "MANDATORY: read only.\n");
    assert_eq!(
        state["evidence"][0]["text"],
        "alpha parser trace is direct evidence.\n"
    );
    assert_eq!(
        state["evidence"][1]["text"],
        "beta renderer path is background.\n"
    );
    let replayed = cli(&["--replay", output.to_str().unwrap()]);
    assert!(
        replayed.status.success(),
        "{}",
        String::from_utf8_lossy(&replayed.stderr)
    );
    let replayed: Value = serde_json::from_slice(&replayed.stdout).unwrap();
    assert_eq!(replayed["verified"], true);
    assert_eq!(replayed["provider_calls"], 0);
    assert_eq!(replayed["complete"], true);
    assert_eq!(replayed["source_freshness"], "not_checked_by_replay");
}

#[tokio::test]
async fn injected_scores_rank_descending_and_ties_follow_bm25() {
    let (repo, manifest) = fixture();
    let plan = shadow::preflight(&manifest, repo.path()).unwrap();
    let baseline = plan["cases"][0]["bm25_ranking"].as_array().unwrap();
    let baseline_ids: Vec<_> = baseline.iter().map(|row| row["id"].clone()).collect();
    let (transport, seen) = probe(Behavior::Ok, &[("alpha", 2.0), ("beta", 2.0)]);
    let provider = jev::Jev::new(transport, manifest.provider_config()).unwrap();
    let output = repo.path().join("tied");
    let report = shadow::campaign(
        manifest,
        repo.path(),
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(seen.lock().unwrap().len(), 1);
    let ranked = report["analysis"]["cases"][0]["jev_ranking"]
        .as_array()
        .unwrap();
    assert_eq!(
        ranked
            .iter()
            .map(|row| row["id"].clone())
            .collect::<Vec<_>>(),
        baseline_ids
    );
    assert_eq!(ranked[0]["score"], 2.0);
    assert_eq!(ranked[1]["score"], 2.0);
    assert_eq!(ranked[0]["rank"], 1);
    assert_eq!(ranked[1]["rank"], 2);
    let request = &seen.lock().unwrap()[0];
    assert_eq!(request["state"]["mandatory"].as_array().unwrap().len(), 1);
    assert_eq!(request["state"]["evidence"].as_array().unwrap().len(), 2);
    assert!(request.get("oracle").is_none());
    assert_eq!(shadow::replay(&output).unwrap()["complete"], true);
}

#[tokio::test]
async fn injected_scores_reverse_bm25_order_without_dropping_source() {
    let (repo, manifest) = fixture();
    let plan = shadow::preflight(&manifest, repo.path()).unwrap();
    let baseline = &plan["cases"][0]["bm25_ranking"];
    assert_eq!(baseline[0]["id"], "alpha");
    assert_eq!(baseline[0]["rank"], 1);
    assert_eq!(baseline[1]["id"], "beta");
    assert_eq!(baseline[1]["rank"], 2);
    let (required, candidates) = manifest.cases[0].packet.context_chunks();
    let (transport, seen) = probe(Behavior::Ok, &[("alpha", 0.0), ("beta", 3.0)]);
    let provider = jev::Jev::new(transport, manifest.provider_config()).unwrap();
    let output = repo.path().join("reversed");
    let report = shadow::campaign(
        manifest,
        repo.path(),
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(report["analysis"]["cases"][0]["bm25_ranking"], *baseline);
    let ranked = &report["analysis"]["cases"][0]["jev_ranking"];
    assert_eq!(ranked[0]["id"], "beta");
    assert_eq!(ranked[0]["score"], 3.0);
    assert_eq!(ranked[0]["rank"], 1);
    assert_eq!(ranked[1]["id"], "alpha");
    assert_eq!(ranked[1]["score"], 0.0);
    assert_eq!(ranked[1]["rank"], 2);
    let requests = seen.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["state"]["mandatory"], json!(required));
    assert_eq!(requests[0]["state"]["evidence"], json!(candidates));
    drop(requests);
    let replayed = shadow::replay(&output).unwrap();
    assert_eq!(replayed["verified"], true);
    assert_eq!(replayed["provider_calls"], 0);
    assert_eq!(replayed["analysis"]["cases"][0]["jev_ranking"], *ranked);
}

#[tokio::test]
async fn failed_prefix_replays_without_continuation_or_new_provider_calls() {
    let (repo, mut manifest) = fixture();
    two_cases(&mut manifest);
    let (transport, seen) = probe(Behavior::Fail, &[]);
    let provider = jev::Jev::new(transport, manifest.provider_config()).unwrap();
    let output = repo.path().join("failed");
    let report = shadow::campaign(
        manifest,
        repo.path(),
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(report["complete"], false);
    assert_eq!(report["error"], "provider_transport");
    assert_eq!(report["reserved_calls"], 1);
    assert_eq!(report["transport_attempts"], 1);
    assert_eq!(report["analysis"]["cases"].as_array().unwrap().len(), 1);
    assert_eq!(report["analysis"]["unknown_usage_calls"], 1);
    let replayed = shadow::replay(&output).unwrap();
    assert_eq!(replayed["verified"], true);
    assert_eq!(replayed["complete"], false);
    assert_eq!(replayed["provider_calls"], 0);
}

#[tokio::test]
async fn injected_provider_pre_admission_refusals_replay_as_reserved_prefixes() {
    let (repo, mut manifest) = fixture();
    two_cases(&mut manifest);
    let (transport, seen) = probe(Behavior::Ok, &[]);
    let mut config = manifest.provider_config();
    config.request_limit = 1;
    let provider = jev::Jev::new(transport, config).unwrap();
    let output = repo.path().join("provider-limit");
    let report = shadow::campaign(
        manifest.clone(),
        repo.path(),
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(report["error"], "provider_request_budget");
    assert_eq!(report["reserved_calls"], 2);
    assert_eq!(report["transport_attempts"], 1);
    let calls = fs::read_to_string(output.join("calls.jsonl")).unwrap();
    let last: Value = serde_json::from_str(calls.lines().last().unwrap()).unwrap();
    assert!(last["receipt"].is_null());
    assert_eq!(shadow::replay(&output).unwrap()["complete"], false);

    let (transport, seen) = probe(Behavior::Ok, &[]);
    let mut config = manifest.provider_config();
    config.questions.insert(
        "aux".into(),
        Question::Noul {
            instructions: "Synthetic auxiliary question".into(),
        },
    );
    let provider = jev::Jev::new(transport, config).unwrap();
    let output = repo.path().join("provider-config");
    let report = shadow::campaign(
        manifest,
        repo.path(),
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(report["error"], "judgment_action_config");
    assert_eq!(report["reserved_calls"], 1);
    assert_eq!(report["transport_attempts"], 0);
    assert_eq!(shadow::replay(&output).unwrap()["complete"], false);
}

#[tokio::test]
async fn cancellation_reserves_interrupted_attempt_and_stops() {
    let (repo, mut manifest) = fixture();
    two_cases(&mut manifest);
    let (mut transport, seen) = probe(Behavior::Hang, &[]);
    let started = Arc::new(tokio::sync::Notify::new());
    transport.started = Some(started.clone());
    let provider = jev::Jev::new(transport, manifest.provider_config()).unwrap();
    let output = repo.path().join("cancelled");
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let canceller = tokio::spawn(async move {
        started.notified().await;
        signal.cancel();
    });
    let report = shadow::campaign(manifest, repo.path(), provider, &output, false, &cancel)
        .await
        .unwrap();
    canceller.await.unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(report["complete"], false);
    assert_eq!(report["error"], "cancelled");
    assert_eq!(report["reserved_calls"], 1);
    assert_eq!(report["transport_attempts"], 1);
    let raw = fs::read_to_string(output.join("calls.jsonl")).unwrap();
    let call: Value = serde_json::from_str(raw.lines().next().unwrap()).unwrap();
    assert_eq!(call["receipt"]["outcome"], "interrupted");
    assert_eq!(shadow::replay(&output).unwrap()["complete"], false);
}

#[tokio::test]
async fn stale_and_tampered_packets_refuse_before_output_or_dispatch() {
    let (repo, manifest) = fixture();
    let (transport, seen) = probe(Behavior::Ok, &[]);
    let provider = jev::Jev::new(transport, manifest.provider_config()).unwrap();
    fs::write(
        repo.path().join("src/evidence.txt"),
        b"MANDATORY: read only.\nalpha parser trace is direct evidence.\nCHANGED outside first excerpt.\n",
    )
    .unwrap();
    let output = repo.path().join("stale");
    assert!(shadow::preflight(&manifest, repo.path()).is_err());
    assert!(
        shadow::campaign(
            manifest.clone(),
            repo.path(),
            provider,
            &output,
            false,
            &CancellationToken::new(),
        )
        .await
        .is_err()
    );
    assert!(!output.exists());
    assert!(seen.lock().unwrap().is_empty());
    fs::write(
        repo.path().join("src/evidence.txt"),
        b"MANDATORY: read only.\nalpha parser trace is direct evidence.\nbeta renderer path is background.\n",
    )
    .unwrap();
    let mut tampered = manifest;
    tampered.cases[0].packet.chunks[0].text = "forged source text\n".into();
    tampered.cases[0].packet.chunks[0].sha256 =
        sha256(tampered.cases[0].packet.chunks[0].text.as_bytes());
    assert!(tampered.validate().is_ok());
    assert!(shadow::preflight(&tampered, repo.path()).is_err());
}

#[test]
fn strict_manifest_modes_limits_and_fresh_output_refusal() {
    let (repo, manifest) = fixture();
    let path = write_manifest(repo.path(), &manifest);
    let mut bad = manifest.clone();
    bad.limits.max_calls = 0;
    assert!(bad.validate().is_err());
    bad.limits.max_calls = 1;
    bad.limits.max_reserved_usd = 0.001;
    assert!(bad.validate().is_err());
    bad.limits.max_reserved_usd = 0.021;
    assert!(bad.validate().is_err());
    let mut duplicate = manifest.clone();
    two_cases(&mut duplicate);
    duplicate.cases[1].id = "trace".into();
    assert!(duplicate.validate().is_err());
    let mut no_required = manifest.clone();
    no_required.cases[0].packet.required.clear();
    assert!(no_required.validate().is_err());
    let mut one_candidate = manifest.clone();
    one_candidate.cases[0].packet.chunks.pop();
    assert!(one_candidate.validate().is_err());
    let malformed = repo.path().join("malformed.json");
    let raw = fs::read_to_string(&path).unwrap();
    fs::write(
        &malformed,
        raw.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
    )
    .unwrap();
    assert!(shadow::read_manifest(&malformed).is_err());
    let mut unknown: Value = decode(&fs::read(&path).unwrap()).unwrap();
    unknown["oracle"] = json!({"secret":"must never enter runner"});
    fs::write(&malformed, encoded(&unknown).unwrap()).unwrap();
    assert!(shadow::read_manifest(&malformed).is_err());
    for args in [
        vec![
            "--manifest",
            path.to_str().unwrap(),
            "--repo",
            repo.path().to_str().unwrap(),
            "--live",
            "--preflight",
        ],
        vec![
            "--replay",
            repo.path().to_str().unwrap(),
            "--manifest",
            path.to_str().unwrap(),
        ],
        vec![
            "--manifest",
            path.to_str().unwrap(),
            "--repo",
            repo.path().to_str().unwrap(),
            "--unknown",
        ],
    ] {
        assert!(!cli(&args).status.success());
    }
    let output = repo.path().join("exists");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("marker"), b"preserved").unwrap();
    let run = cli(&[
        "--manifest",
        path.to_str().unwrap(),
        "--repo",
        repo.path().to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    assert!(!run.status.success());
    assert_eq!(fs::read(output.join("marker")).unwrap(), b"preserved");
}

#[test]
fn full_scoring_batch_must_fit_the_existing_jev_capacity_admission() {
    let (repo, _) = fixture();
    let mut seed = 0x1234_5678_u32;
    let noisy = |seed: &mut u32| -> Vec<u8> {
        (0..70_000)
            .map(|_| {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 17;
                *seed ^= *seed << 5;
                b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                    [(*seed as usize) % 62]
            })
            .collect()
    };
    let mut bytes = b"mandatory\n".to_vec();
    bytes.extend(noisy(&mut seed));
    bytes.push(b'\n');
    bytes.extend(noisy(&mut seed));
    bytes.push(b'\n');
    fs::write(repo.path().join("src/noisy.txt"), bytes).unwrap();
    git(repo.path(), &["add", "src/noisy.txt"]);
    git(repo.path(), &["commit", "-qm", "noisy source"]);
    let revision = git(repo.path(), &["rev-parse", "HEAD"]);
    let reference = |id: &str, line| packet::SourceRef {
        id: id.into(),
        path: "src/noisy.txt".into(),
        start_line: line,
        end_line: line,
    };
    let packet = packet::build(
        repo.path(),
        &packet::Spec {
            version: 1,
            source_revision: revision,
            task: "Inspect synthetic data".into(),
            required: vec![reference("policy", 1)],
            chunks: vec![reference("first", 2), reference("second", 3)],
        },
    )
    .unwrap();
    let manifest = shadow::Manifest {
        version: 1,
        limits: shadow::Limits {
            max_calls: 1,
            max_reserved_usd: 0.01,
        },
        cases: vec![shadow::Case {
            id: "noise".into(),
            packet,
        }],
    };
    manifest.validate().unwrap();
    let error = shadow::preflight(&manifest, repo.path()).unwrap_err();
    assert!(error.0.starts_with("context_capacity_"), "{error}");
}

#[tokio::test]
async fn replay_rejects_modified_ranks_receipts_usage_and_pending_checkpoint() {
    let (repo, manifest) = fixture();
    let (transport, _) = probe(Behavior::Ok, &[("alpha", 3.0), ("beta", 1.0)]);
    let provider = jev::Jev::new(transport, manifest.provider_config()).unwrap();
    let output = repo.path().join("tamper");
    shadow::campaign(
        manifest,
        repo.path(),
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let report_path = output.join("report.json");
    let original_report = fs::read(&report_path).unwrap();
    let mut report: Value = decode(&original_report).unwrap();
    report["analysis"]["cases"][0]["jev_ranking"][0]["score"] = json!(0.0);
    fs::write(&report_path, encoded(&report).unwrap()).unwrap();
    assert!(shadow::replay(&output).is_err());
    fs::write(&report_path, &original_report).unwrap();
    report = decode(&original_report).unwrap();
    report["pending"] = json!("trace");
    fs::write(&report_path, encoded(&report).unwrap()).unwrap();
    assert!(shadow::replay(&output).is_err());
    fs::write(&report_path, &original_report).unwrap();
    let call_path = output.join("calls.jsonl");
    let original_calls = fs::read(&call_path).unwrap();
    let mut call: Value =
        serde_json::from_slice(&original_calls[..original_calls.len() - 1]).unwrap();
    call["receipt"]["request_sha256"] = json!("0".repeat(64));
    fs::write(
        &call_path,
        format!("{}\n", String::from_utf8(encoded(&call).unwrap()).unwrap()),
    )
    .unwrap();
    assert!(shadow::replay(&output).is_err());
    call = serde_json::from_slice(&original_calls[..original_calls.len() - 1]).unwrap();
    call["receipt"]["usage"]["input_tokens"] = json!(999);
    fs::write(
        &call_path,
        format!("{}\n", String::from_utf8(encoded(&call).unwrap()).unwrap()),
    )
    .unwrap();
    assert!(shadow::replay(&output).is_err());
    call = serde_json::from_slice(&original_calls[..original_calls.len() - 1]).unwrap();
    call["receipt"]["response"] = json!("{\"model\":\"wrong\"}");
    fs::write(
        &call_path,
        format!("{}\n", String::from_utf8(encoded(&call).unwrap()).unwrap()),
    )
    .unwrap();
    assert!(shadow::replay(&output).is_err());
    fs::write(&call_path, original_calls).unwrap();
    assert_eq!(shadow::replay(&output).unwrap()["verified"], true);
}
