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
        baseline: None,
        diagnostic: None,
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

fn coding_profile() -> codex::Profile {
    codex::Profile {
        model: "synthetic-coder".into(),
        effort: "low".into(),
        cli_version: "codex-cli 0.154.0".into(),
        cli_sha256: "a".repeat(64),
        catalog: json!({"models":[{"slug":"synthetic-coder","apply_patch_tool_type":null,
            "experimental_supported_tools":[],"node_repl_disabled":true}]}),
    }
}

fn mixed_inputs() -> (Manifest, Oracle) {
    let (mut manifest, mut oracle) = inputs();
    manifest.version = 2;
    manifest.baseline = Some("bm25_v1".into());
    manifest.diagnostic = Some(coding_profile());
    manifest.limits.max_reserved_usd = 0.006;
    oracle.manifest_sha256 = manifest.digest().unwrap();
    (manifest, oracle)
}

struct Coding {
    seen: Arc<Mutex<Vec<Value>>>,
    fail: bool,
}
#[async_trait]
impl codex::Transport for Coding {
    async fn run(&mut self, body: &Value, _: &CancellationToken) -> Result<codex::Transcript> {
        self.seen.lock().unwrap().push(body.clone());
        let choice = if body["state"]["evidence"].to_string().contains("needle") {
            "right"
        } else {
            "wrong"
        };
        let item = if self.fail {
            json!({"type":"command_execution","command":"forbidden"})
        } else {
            json!({"type":"agent_message","text":json!({"choice":choice}).to_string()})
        };
        Ok(codex::Transcript {
            stdout: [
                json!({"type":"thread.started"}),
                json!({"type":"turn.started"}),
                json!({"type":"item.completed","item":item}),
                json!({"type":"turn.completed","usage":{
                    "input_tokens":101,"cached_input_tokens":50,"output_tokens":12}}),
            ]
            .iter()
            .map(|e| format!("{e}\n"))
            .collect(),
            stderr: String::new(),
            exit_code: Some(0),
            stop: None,
        })
    }
}

#[test]
fn lexical_baseline_uses_task_identifiers_and_stable_ties_without_choice_labels() {
    let (mut manifest, _) = mixed_inputs();
    let case = &mut manifest.cases[0];
    case.task = "Verify cached metadata hash".into();
    case.chunks = vec![
        chunk("a", "General background."),
        chunk("b", "fn verify_cached_metadata_hash() {}"),
        chunk("c", "General background."),
    ];
    let ranked = lexical::ranking(case);
    assert_eq!(
        ranked.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        ["b", "a", "c"]
    );
    assert!(ranked[0].1 > 0.0);
    case.diagnoses
        .insert("bait".into(), "General background".repeat(100));
    assert_eq!(lexical::ranking(case), ranked);
    case.chunks[0] = chunk(
        "a",
        &format!("verify_cached_metadata_hash {}", "padding ".repeat(100)),
    );
    assert_eq!(lexical::ranking(case)[0].0, "b");
    case.task = "No matching terms".into();
    assert_eq!(
        lexical::ranking(case)
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
}

#[tokio::test]
async fn mixed_provider_pairing_replays_with_separate_costs_and_no_oracle_leak() {
    let (manifest, oracle) = mixed_inputs();
    let mut requests = Vec::new();
    let temp = tempfile::tempdir().unwrap();
    for changed in [false, true] {
        let mut oracle = oracle.clone();
        if changed {
            for truth in oracle.cases.values_mut() {
                truth.diagnosis = "wrong".into();
            }
        }
        let (provider, jev_seen) = provider(&manifest, false, false);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let diagnostic = codex::Diagnostic::new(
            Coding {
                seen: seen.clone(),
                fail: false,
            },
            coding_profile(),
        )
        .unwrap();
        let output = temp
            .path()
            .join(if changed { "changed" } else { "original" });
        let report = campaign_with_diagnostic(
            manifest.clone(),
            oracle,
            provider,
            diagnostic,
            &output,
            false,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(report["complete"], true);
        assert_eq!(report["live_calls"], 0);
        assert_eq!(report["analysis"]["jev_attempts"], 2);
        assert_eq!(report["analysis"]["codex_turn_attempts"], 4);
        assert_eq!(report["reserved_codex_turns"], 4);
        assert_eq!(report["reserved_usd"], json!(2.0 * per_call_usd()));
        let coding_requests = seen.lock().unwrap().clone();
        assert_eq!(coding_requests.len(), 4);
        for request in &coding_requests {
            assert!(
                request["state"]["mandatory"]
                    .to_string()
                    .contains("MANDATORY_POLICY")
            );
            assert!(!request.to_string().contains("GRADER_ONLY_CANARY"));
            assert!(request["state"].get("split").is_none());
        }
        for row in report["analysis"]["rows"].as_array().unwrap() {
            let treatment = row["arm"] == "treatment";
            assert_eq!(row["correct"], treatment != changed);
            assert_eq!(row["confidence"], Value::Null);
        }
        for row in report["analysis"]["summary"].as_array().unwrap() {
            assert_eq!(row["estimated_usd"], Value::Null);
            assert_eq!(row["codex_cached_input_tokens"], 50);
        }
        assert_eq!(replay(&output).unwrap()["analysis"], report["analysis"]);
        requests.push((jev_seen.lock().unwrap().clone(), coding_requests));
    }
    assert_eq!(requests[0], requests[1]);
}

#[tokio::test]
async fn unexpected_coding_tool_stops_before_any_selection_and_preserves_reservation() {
    let (manifest, oracle) = mixed_inputs();
    let (provider, jev_seen) = provider(&manifest, false, false);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let diagnostic = codex::Diagnostic::new(
        Coding {
            seen: seen.clone(),
            fail: true,
        },
        coding_profile(),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("failure");
    let report = campaign_with_diagnostic(
        manifest,
        oracle,
        provider,
        diagnostic,
        &output,
        false,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(report["complete"], false);
    assert_eq!(report["error"], "codex_unexpected_event");
    assert_eq!(report["reserved_codex_turns"], 1);
    assert_eq!(report["reserved_usd"], 0.0);
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert!(jev_seen.lock().unwrap().is_empty());
    assert_eq!(replay(&output).unwrap()["verified"], true);
}

#[tokio::test]
async fn coding_transcript_rejects_missing_usage_extra_answers_and_unoffered_choice() {
    use codex::Transport;
    let body = json!({"choices":{"insufficient":"missing evidence"}});
    let original = codex::Mock
        .run(&body, &CancellationToken::new())
        .await
        .unwrap();
    assert!(codex::parse(&original, &body).is_ok());
    for stdout in [
        original.stdout.replace("\"cached_input_tokens\":0,", ""),
        original.stdout.replace("insufficient", "unoffered"),
        original.stdout.clone() + "{\"type\":\"turn.started\"}\n",
        original
            .stdout
            .replace("\"output_tokens\":10", "\"output_tokens\":-1"),
    ] {
        let mut transcript = original.clone();
        transcript.stdout = stdout;
        assert!(codex::parse(&transcript, &body).is_err());
    }
}

#[tokio::test]
async fn coding_turn_cap_and_pending_receipt_prevent_extra_dispatch() {
    let profile = coding_profile();
    let body = codex::request(
        &profile,
        json!({}),
        &BTreeMap::from([("insufficient".into(), "missing evidence".into())]),
    )
    .unwrap();
    let mut diagnostic = codex::Diagnostic::new(codex::Mock, profile).unwrap();
    let cancel = CancellationToken::new();
    for i in 1..=8 {
        diagnostic.ask(body.clone(), &cancel).await.unwrap();
        assert!(diagnostic.ask(body.clone(), &cancel).await.is_err());
        assert_eq!(diagnostic.calls(), i);
        assert!(diagnostic.take_evidence().is_some());
    }
    assert!(diagnostic.ask(body, &cancel).await.is_err());
    assert_eq!(diagnostic.calls(), 8);
    assert!(diagnostic.take_evidence().is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn coding_cli_enforces_eof_tool_settings_identity_output_bounds_and_cancellation() {
    use codex::Transport;
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let executable = temp.path().join("fake-codex");
    let pid_path = temp.path().join("child.pid");
    let body = json!({"choices":{"insufficient":"missing evidence"}});
    for mode in ["normal", "oversize", "encoding", "cancel"] {
        let script = format!(
            r#"#!/usr/bin/python3
import json,os,sys,time
from pathlib import Path
args=sys.argv[1:]
assert '--ignore-user-config' in args and '--ignore-rules' in args
assert 'features.shell_tool=false' in args and 'features.multi_agent=false' in args
assert 'features.code_mode=false' in args and 'web_search="disabled"' in args
assert 'TYPESAFE_API_KEY' not in os.environ
assert sorted(p.name for p in Path.cwd().iterdir()) == ['catalog.json','instructions.md','schema.json']
assert 'choices' in json.load(sys.stdin)
mode={mode:?}
if mode=='cancel':
    Path({pid:?}).write_text(str(os.getpid()))
    time.sleep(30)
if mode=='oversize':
    os.write(1,b'x'*40000); sys.exit(0)
if mode=='encoding':
    os.write(1,b'\xff'*30000); sys.exit(0)
for event in [{{'type':'thread.started'}},{{'type':'turn.started'}},
              {{'type':'item.completed','item':{{'type':'agent_message','text':'{{"choice":"insufficient"}}'}}}},
              {{'type':'turn.completed','usage':{{'input_tokens':10,'cached_input_tokens':0,'output_tokens':2}}}}]:
    print(json.dumps(event))
"#,
            pid = pid_path.to_str().unwrap()
        );
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut profile = coding_profile();
        assert!(codex::Cli::new(executable.clone(), profile.clone()).is_err());
        profile.cli_sha256 = codex::binary_digest(&executable).unwrap();
        let mut cli = codex::Cli::new(executable.clone(), profile).unwrap();
        let cancel = CancellationToken::new();
        let trigger = cancel.clone();
        let pid = pid_path.clone();
        let watcher = tokio::spawn(async move {
            if mode == "cancel" {
                while !pid.exists() {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
                trigger.cancel();
            }
        });
        let transcript =
            tokio::time::timeout(std::time::Duration::from_secs(5), cli.run(&body, &cancel))
                .await
                .unwrap()
                .unwrap();
        watcher.await.unwrap();
        assert!(transcript.stdout.len() <= codex::MAX_STDOUT);
        assert!(transcript.stderr.len() <= codex::MAX_STDERR);
        if mode == "normal" {
            assert!(codex::parse(&transcript, &body).is_ok(), "{transcript:?}");
        } else {
            assert!(transcript.stop.is_some(), "{transcript:?}");
            assert!(codex::parse(&transcript, &body).is_err());
        }
        if mode == "cancel" {
            assert_eq!(transcript.stop.as_deref(), Some("cancelled"));
            let pid = std::fs::read_to_string(&pid_path).unwrap();
            #[cfg(target_os = "linux")]
            assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        }
    }
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
