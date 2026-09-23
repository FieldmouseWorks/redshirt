use async_trait::async_trait;
use redshirt::{
    CancellationToken, Provider, Result,
    comparison::Split,
    context::*,
    decision::{Question, Questions},
    evidence::{decode, encoded, sha256},
    jev,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

fn chunk(id: &str, text: &str) -> Chunk {
    Chunk {
        id: id.into(),
        source: format!("synthetic:{id}"),
        text: text.into(),
        sha256: sha256(text.as_bytes()),
    }
}
fn inputs() -> (Manifest, Oracle) {
    let cases = [Split::Calibration, Split::HeldOut]
        .into_iter()
        .enumerate()
        .map(|(i, split)| Case {
            id: format!("c{i}"),
            split,
            task: "Diagnose the marked observation from the supplied evidence.".into(),
            mandatory: vec![chunk(
                "owner",
                "Immutable owner packet. Never mutate the environment.",
            )],
            chunks: vec![
                chunk("a", "Unrelated background."),
                chunk("b", "Essential measured needle."),
                chunk("c", "Other background."),
            ],
            baseline_order: vec!["a".into(), "b".into(), "c".into()],
            diagnoses: BTreeMap::from([
                ("right".into(), "Needle is measured.".into()),
                ("wrong".into(), "Needle is absent.".into()),
                ("insufficient".into(), "Evidence is insufficient.".into()),
            ]),
        })
        .collect();
    let manifest = Manifest {
        version: 1,
        source_revision: "a".repeat(40),
        required: vec![chunk("policy", "MANDATORY_POLICY")],
        limits: Limits {
            context_bytes: 4096,
            selected_chunks: 1,
            request_bytes: 8192,
            max_calls: 6,
            max_reserved_usd: 0.04,
        },
        cases,
    };
    let oracle = Oracle {
        manifest_sha256: manifest.digest().unwrap(),
        cases: manifest
            .cases
            .iter()
            .map(|c| {
                (
                    c.id.clone(),
                    Truth {
                        diagnosis: "right".into(),
                        essential: vec!["b".into()],
                        proof: vec!["GRADER_ONLY_CANARY".into()],
                    },
                )
            })
            .collect(),
    };
    (manifest, oracle)
}

struct Synthetic {
    seen: Arc<Mutex<Vec<Value>>>,
    malformed: bool,
    hang: bool,
}
#[async_trait]
impl jev::Transport for Synthetic {
    async fn post(&mut self, bytes: Vec<u8>) -> Result<(u16, Vec<u8>)> {
        let request = decode(&bytes)?;
        self.seen.lock().unwrap().push(request.clone());
        if self.hang {
            std::future::pending::<()>().await;
        }
        let questions: Questions = serde_json::from_value(request["questions"].clone()).unwrap();
        let mut answers = serde_json::Map::new();
        for (id, question) in questions {
            let answer = match question {
                Question::Choice { criteria, .. } => {
                    let text = request["state"]["evidence"].to_string();
                    let choice = if text.contains("needle") {
                        "right"
                    } else {
                        "wrong"
                    };
                    let probabilities: BTreeMap<_, _> = criteria
                        .keys()
                        .map(|k| (k.clone(), if k == choice { 1.0 } else { 0.0 }))
                        .collect();
                    json!({"type":"choice","choice":choice,"probabilities":probabilities,"confidence":1.0})
                }
                Question::Score { criteria, .. } => {
                    let level = if id == "b" { 3 } else { 0 };
                    let legend: BTreeMap<_, _> = criteria
                        .into_iter()
                        .enumerate()
                        .map(|(i, s)| (i.to_string(), s))
                        .collect();
                    json!({"type":"score","score":level as f64,"legend":legend,
                        "probabilities":{level.to_string():1.0},"confidence":1.0})
                }
                Question::Noul { .. } => json!({"type":"noul","noul":0.5}),
            };
            answers.insert(id, answer);
        }
        if self.malformed && answers.contains_key("b") {
            answers.insert("policy".into(), json!({"type":"noul","noul":0.0}));
        }
        Ok((
            200,
            encoded(&json!({"model":jev::MODEL,"answers":answers,
            "usage":{"input_tokens":123,"output_tokens":17}}))?,
        ))
    }
}
fn provider(
    manifest: &Manifest,
    malformed: bool,
    hang: bool,
) -> (jev::Jev<Synthetic>, Arc<Mutex<Vec<Value>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    (
        jev::Jev::new(
            Synthetic {
                seen: seen.clone(),
                malformed,
                hang,
            },
            manifest.provider_config(),
        )
        .unwrap(),
        seen,
    )
}

#[tokio::test]
async fn paired_packets_preserve_policy_exclude_truth_and_replay_without_a_provider() {
    let (manifest, oracle) = inputs();
    let (provider, seen) = provider(&manifest, false, false);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("proof");
    let report = campaign(
        manifest.clone(),
        oracle.clone(),
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(report["quality_evidence"], false);
    assert_eq!(report["live_calls"], 0);
    assert_eq!(report["transport_attempts"], 6);
    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests.len(), 6);
    for request in &requests {
        assert!(
            request["state"]["mandatory"]
                .to_string()
                .contains("MANDATORY_POLICY")
        );
        assert!(!request.to_string().contains("GRADER_ONLY_CANARY"));
        assert!(request["state"].get("split").is_none());
    }
    for row in report["analysis"]["rows"].as_array().unwrap() {
        assert_eq!(row["correct"], row["arm"] == "treatment");
        assert_eq!(
            row["missing_essential"],
            if row["arm"] == "treatment" {
                json!([])
            } else {
                json!(["b"])
            }
        );
    }
    let replayed = replay(&output).unwrap();
    assert_eq!(replayed["provider_calls"], 0);
    assert_eq!(replayed["analysis"], report["analysis"]);
    assert_eq!(seen.lock().unwrap().len(), 6);
    assert!(
        campaign(
            manifest.clone(),
            oracle.clone(),
            self::provider(&manifest, false, false).0,
            &output,
            false,
            &CancellationToken::new()
        )
        .await
        .is_err()
    );

    // Metamorphic negative control: changing the grading key changes grades,
    // while every outbound request stays byte-for-byte identical.
    let mut changed = oracle;
    for truth in changed.cases.values_mut() {
        truth.diagnosis = "wrong".into();
    }
    let (provider, changed_seen) = self::provider(&manifest, false, false);
    let changed_report = campaign(
        manifest,
        changed,
        provider,
        &temp.path().join("changed"),
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(*changed_seen.lock().unwrap(), requests);
    assert_ne!(
        changed_report["analysis"]["rows"],
        report["analysis"]["rows"]
    );
}

#[tokio::test]
async fn canonical_oracle_size_is_admitted_before_dispatch_and_replays() {
    let (manifest, mut oracle) = inputs();
    let unicode_proof = "é".repeat(800); // 1,600 UTF-8 bytes; 4,800 encoded bytes.
    for truth in oracle.cases.values_mut() {
        truth.proof = vec![unicode_proof.clone(); 4];
    }
    assert!(serde_json::to_vec(&oracle).unwrap().len() < 32768);
    assert!(encoded(&oracle).unwrap().len() > 32768);
    assert_eq!(
        preflight(&manifest, &oracle).unwrap_err().0,
        "context_oracle_size"
    );

    let (provider, seen) = provider(&manifest, false, false);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("oversized");
    assert_eq!(
        campaign(
            manifest.clone(),
            oracle.clone(),
            provider,
            &output,
            false,
            &CancellationToken::new(),
        )
        .await
        .unwrap_err()
        .0,
        "context_oracle_size"
    );
    assert!(seen.lock().unwrap().is_empty());
    assert!(!output.exists());

    for truth in oracle.cases.values_mut() {
        truth.proof.pop();
    }
    assert!(encoded(&oracle).unwrap().len() <= 32768);
    let (provider, seen) = self::provider(&manifest, false, false);
    let output = temp.path().join("within_limit");
    let report = campaign(
        manifest,
        oracle.clone(),
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(
        std::fs::read(output.join("oracle.json")).unwrap(),
        encoded(&oracle).unwrap()
    );
    assert_eq!(replay(&output).unwrap()["provider_calls"], 0);
    assert_eq!(seen.lock().unwrap().len(), 6);
}

#[tokio::test]
async fn malformed_selector_stops_before_treatment_and_keeps_unknown_usage() {
    let (manifest, oracle) = inputs();
    let (provider, seen) = provider(&manifest, true, false);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("proof");
    let report = campaign(
        manifest,
        oracle,
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["error"], "invalid_answer_ids");
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(report["reserved_calls"], 2);
    assert_eq!(report["analysis"]["summary"][1]["unknown_usage_calls"], 1);
    assert_eq!(report["analysis"]["summary"][1]["ungraded"], 1);
    assert_eq!(replay(&output).unwrap()["verified"], true);
}

#[tokio::test]
async fn cancellation_preserves_interrupted_call_and_its_reservation() {
    let (manifest, oracle) = inputs();
    let (provider, seen) = provider(&manifest, false, true);
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let observed = seen.clone();
    let task = tokio::spawn(async move {
        while observed.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
        trigger.cancel();
    });
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("proof");
    let report = campaign(manifest, oracle, provider, &output, false, &cancel)
        .await
        .unwrap();
    task.await.unwrap();
    assert_eq!(report["error"], "cancelled");
    assert_eq!(report["reserved_calls"], 1);
    assert_eq!(report["reserved_usd"], json!(per_call_usd()));
    assert_eq!(report["analysis"]["summary"][0]["unknown_usage_calls"], 1);
    assert_eq!(replay(&output).unwrap()["verified"], true);
}

#[tokio::test]
async fn invalid_inputs_and_exhausted_judgment_budget_make_no_extra_dispatch() {
    let (mut manifest, oracle) = inputs();
    manifest.limits.max_calls = 5;
    assert!(manifest.validate().is_err());
    manifest.limits.max_calls = 6;
    manifest.required.clear();
    assert!(manifest.validate().is_err());
    let (manifest, mut oracle2) = inputs();
    oracle2.manifest_sha256 = "0".repeat(64);
    assert!(preflight(&manifest, &oracle2).is_err());
    let mut too_small = manifest.clone();
    too_small.limits.context_bytes = 1024;
    too_small.required[0] = chunk("policy", &"x".repeat(1500));
    assert!(too_small.validate().is_err());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut provider = jev::Jev::new(
        Synthetic {
            seen: seen.clone(),
            malformed: false,
            hang: false,
        },
        jev::Config {
            request_limit: 1,
            ..jev::Config::default()
        },
    )
    .unwrap();
    let questions = BTreeMap::from([(
        "check".into(),
        Question::Noul {
            instructions: "Is it visible?".into(),
        },
    )]);
    let mut too_many = questions.clone();
    for i in 0..8 {
        too_many.insert(format!("q{i}"), questions["check"].clone());
    }
    assert!(provider.ask(json!({}), too_many).await.is_err());
    assert_eq!(provider.calls(), 0);
    provider.ask(json!({}), questions.clone()).await.unwrap();
    assert_eq!(
        provider
            .ask(json!({}), questions.clone())
            .await
            .unwrap_err()
            .0,
        "provider_receipt_pending"
    );
    assert_eq!(provider.take_evidence().len(), 1);
    assert_eq!(
        provider.ask(json!({}), questions).await.unwrap_err().0,
        "provider_request_budget"
    );
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert!(oracle.validate(&manifest).is_ok());
}

#[tokio::test]
async fn replay_rejects_changed_request_grades_and_accounting() {
    let (manifest, oracle) = inputs();
    let (provider, _) = provider(&manifest, false, false);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("proof");
    campaign(
        manifest,
        oracle,
        provider,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let report_path = output.join("report.json");
    let original = std::fs::read(&report_path).unwrap();
    let mut changed = decode(&original).unwrap();
    changed["analysis"]["rows"][0]["correct"] = json!(true);
    std::fs::write(&report_path, encoded(&changed).unwrap()).unwrap();
    assert!(replay(&output).is_err());
    for (field, value) in [
        ("reserved_usd", json!(0.0)),
        ("reserved_calls", json!(0)),
        ("transport_attempts", json!(0)),
        ("live_calls", json!(6)),
        ("quality_evidence", json!(true)),
    ] {
        let mut changed = decode(&original).unwrap();
        changed[field] = value;
        std::fs::write(&report_path, encoded(&changed).unwrap()).unwrap();
        assert!(replay(&output).is_err(), "accepted changed {field}");
    }
    std::fs::write(&report_path, original).unwrap();
    let calls_path = output.join("calls.jsonl");
    let calls = std::fs::read_to_string(&calls_path).unwrap();
    let mut lines: Vec<_> = calls
        .lines()
        .map(|line| decode(line.as_bytes()).unwrap())
        .collect();
    lines[0]["receipt"]["request"]["state"]["mandatory"] = json!([]);
    lines[0]["receipt"]["request_sha256"] =
        json!(sha256(&encoded(&lines[0]["receipt"]["request"]).unwrap()));
    let changed = lines
        .iter()
        .map(|v| String::from_utf8(encoded(v).unwrap()).unwrap() + "\n")
        .collect::<String>();
    std::fs::write(calls_path, changed).unwrap();
    assert!(replay(&output).is_err());
}
