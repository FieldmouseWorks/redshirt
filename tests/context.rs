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
        context_policy: Some(CONTEXT_POLICY.into()),
        baseline: Some("bm25_v1".into()),
        diagnostic: None,
        version: 3,
        source_revision: "a".repeat(40),
        required: vec![chunk("policy", "MANDATORY_POLICY")],
        limits: Limits {
            context_bytes: None,
            selected_chunks: None,
            request_bytes: None,
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

fn legacy_inputs() -> (Manifest, Oracle) {
    let (mut manifest, mut oracle) = inputs();
    manifest.version = 1;
    manifest.context_policy = None;
    manifest.baseline = None;
    manifest.limits.context_bytes = Some(4096);
    manifest.limits.selected_chunks = Some(1);
    manifest.limits.request_bytes = Some(8192);
    oracle.manifest_sha256 = manifest.digest().unwrap();
    (manifest, oracle)
}

fn mixed_inputs() -> (Manifest, Oracle) {
    let (mut manifest, mut oracle) = inputs();
    manifest.diagnostic = Some(coding_profile());
    manifest.limits.max_reserved_usd = 0.006;
    oracle.manifest_sha256 = manifest.digest().unwrap();
    (manifest, oracle)
}

fn large_codex_inputs(repetitions: usize) -> (Manifest, Oracle) {
    // Upgrade the saved pre-v3 mock inputs, then use a long, highly repetitive
    // required chunk. The independent review probe found this packet shape.
    let mut manifest: Manifest =
        serde_json::from_str(include_str!("fixtures/context-v2-saved/manifest.json")).unwrap();
    let mut oracle: Oracle =
        serde_json::from_str(include_str!("fixtures/context-v2-saved/oracle.json")).unwrap();
    manifest.version = 3;
    manifest.context_policy = Some(CONTEXT_POLICY.into());
    manifest.limits.context_bytes = None;
    manifest.limits.selected_chunks = None;
    manifest.limits.request_bytes = None;
    manifest.required[0] = chunk(
        "policy",
        &format!("{}x", "-".repeat(64)).repeat(repetitions),
    );
    oracle.manifest_sha256 = manifest.digest().unwrap();
    (manifest, oracle)
}

fn maximum_bounded_transcript() -> codex::Transcript {
    let mut events = [
        json!({"type":"thread.started"}),
        json!({"type":"turn.started"}),
        json!({"type":"item.completed","item":{"type":"agent_message",
            "text":"{\"choice\":\"insufficient\"}"}}),
        json!({"type":"turn.completed","usage":{
            "input_tokens":100,"cached_input_tokens":0,"output_tokens":1,"padding":""}}),
    ];
    let unpadded = events
        .iter()
        .map(|event| format!("{event}\n"))
        .collect::<String>();
    events[3]["usage"]["padding"] = "\u{7f}".repeat(codex::MAX_STDOUT - unpadded.len()).into();
    let stdout = events.iter().map(|event| format!("{event}\n")).collect();
    let transcript = codex::Transcript {
        stdout,
        stderr: "\u{7f}".repeat(codex::MAX_STDERR),
        exit_code: Some(0),
        stop: None,
    };
    assert_eq!(transcript.stdout.len(), codex::MAX_STDOUT);
    assert_eq!(transcript.stderr.len(), codex::MAX_STDERR);
    transcript
}

struct BoundedCoding {
    seen: Arc<Mutex<Vec<Value>>>,
}
#[async_trait]
impl codex::Transport for BoundedCoding {
    async fn run(&mut self, body: &Value, _: &CancellationToken) -> Result<codex::Transcript> {
        self.seen.lock().unwrap().push(body.clone());
        Ok(maximum_bounded_transcript())
    }
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
            assert_eq!(row["correct"], !changed);
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

fn noise(length: usize) -> String {
    let mut seed = 0x8765_4321u32;
    (0..length)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"[(seed as usize) % 62]
                as char
        })
        .collect()
}

fn wide_inputs(payload: &str) -> (Manifest, Oracle) {
    let (mut manifest, mut oracle) = inputs();
    for case in &mut manifest.cases {
        case.chunks = (0..8)
            .map(|i| {
                chunk(
                    &format!("e{i}"),
                    &format!("Essential measured needle. {payload}"),
                )
            })
            .collect();
        case.baseline_order = (0..8).rev().map(|i| format!("e{i}")).collect();
        oracle.cases.get_mut(&case.id).unwrap().essential =
            (0..8).map(|i| format!("e{i}")).collect();
    }
    oracle.manifest_sha256 = manifest.digest().unwrap();
    (manifest, oracle)
}

#[tokio::test]
async fn v3_keeps_all_eight_excerpts_and_admits_large_escaped_packets() {
    let payload = "é \" \\ <|endoftext|> ".repeat(200);
    let (mut manifest, mut oracle) = wide_inputs(&payload);
    for i in 1..5 {
        manifest
            .required
            .push(chunk(&format!("policy{i}"), "Retain this owner policy."));
        manifest.cases[0]
            .mandatory
            .push(chunk(&format!("owner{i}"), "Retain this case rule."));
    }
    manifest.cases[0].task = "Read only diagnosis. ".repeat(220);
    manifest.cases[0].chunks[0] = chunk(
        "e0",
        &format!("Essential measured needle. {}", "stable ".repeat(2600)),
    );
    assert!(manifest.cases[0].task.len() > 4096);
    assert!(manifest.cases[0].chunks[0].text.len() > 16384);
    assert_eq!(manifest.required.len(), 5);
    assert_eq!(manifest.cases[0].mandatory.len(), 5);
    oracle.manifest_sha256 = manifest.digest().unwrap();
    let plan = preflight(&manifest, &oracle).unwrap();
    assert_eq!(plan["capacity_policy"]["context_policy"], CONTEXT_POLICY);
    assert_eq!(
        plan["capacity_policy"]["estimator"],
        "tiktoken-rs_0.12.0_max_r50k_cl100k_o200k_ordinary_json_v1"
    );
    assert_eq!(plan["capacity_policy"]["published_total_tokens"], 64000);
    assert_eq!(
        plan["capacity_policy"]["published_state_plus_longest_question_tokens"],
        32000
    );
    assert_eq!(plan["capacity_policy"]["headroom_percent"], 20);
    assert_eq!(plan["capacity_policy"]["admit_total_tokens"], 51200);
    assert_eq!(
        plan["capacity_policy"]["admit_state_plus_longest_question_tokens"],
        25600
    );
    assert_eq!(
        plan["capacity_policy"]["exact_jev_tokenization_known"],
        false
    );
    assert!(
        plan["cases"][0]["jev_capacity"]["selection"]["total_estimated_tokens"]
            .as_u64()
            .unwrap()
            <= 51200
    );
    assert!(plan["cases"][0]["jev_capacity"]["selection"]["state_plus_longest_question_estimated_tokens"]
        .as_u64().unwrap() <= 25600);
    let (provider, seen) = provider(&manifest, false, false);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("wide");
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
    assert_eq!(report["capacity_policy"], plan["capacity_policy"]);
    assert_eq!(report["capacity_cases"], plan["cases"]);
    assert_eq!(report["complete"], true);
    assert_eq!(replay(&output).unwrap()["provider_calls"], 0);
    for body in seen.lock().unwrap().iter() {
        let evidence = body["state"]["evidence"].as_array().unwrap();
        assert_eq!(evidence.len(), 8);
        assert_eq!(
            body["state"]["mandatory"].as_array().unwrap().len(),
            if body["state"]["task"].as_str().unwrap().len() > 4096 {
                10
            } else {
                6
            }
        );
        assert!(
            evidence
                .iter()
                .any(|c| c["text"].as_str().unwrap().contains("<|endoftext|>"))
        );
        assert_eq!(
            evidence
                .iter()
                .map(|c| c["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["e0", "e1", "e2", "e3", "e4", "e5", "e6", "e7"]
        );
        assert!(encoded(body).unwrap().len() > 32768);
    }
    for row in report["analysis"]["rows"].as_array().unwrap() {
        assert_eq!(row["selected"].as_array().unwrap().len(), 8);
        assert_eq!(row["missing_essential"], json!([]));
    }
}

#[tokio::test]
async fn late_case_single_question_overflow_rejects_whole_campaign() {
    let (mut control, mut oracle) = wide_inputs(&noise(3500));
    preflight(&control, &oracle).unwrap();
    for chunk in &mut control.cases[1].chunks {
        *chunk = self::chunk(
            &chunk.id,
            &format!("Essential measured needle. {}", noise(5600)),
        );
    }
    oracle.manifest_sha256 = control.digest().unwrap();
    assert_eq!(
        preflight(&control, &oracle).unwrap_err().0,
        "context_capacity_single"
    );
    let (provider, seen) = provider(&control, false, false);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("rejected");
    assert_eq!(
        campaign(
            control,
            oracle,
            provider,
            &output,
            false,
            &CancellationToken::new()
        )
        .await
        .unwrap_err()
        .0,
        "context_capacity_single"
    );
    assert!(seen.lock().unwrap().is_empty());
    assert!(!output.exists());
}

#[test]
fn v3_rejects_legacy_caps_and_explicit_null() {
    let (manifest, _) = inputs();
    for field in ["context_bytes", "selected_chunks", "request_bytes"] {
        let mut value = serde_json::to_value(&manifest).unwrap();
        value["limits"][field] = json!(2048);
        let changed: Manifest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(changed.validate().unwrap_err().0, "context_limits");
        value["limits"][field] = Value::Null;
        assert!(serde_json::from_value::<Manifest>(value).is_err());
    }
    let mut changed = manifest;
    changed.context_policy = Some("jev_1_13_exact_v1".into());
    assert_eq!(changed.validate().unwrap_err().0, "context_protocol");
}

#[test]
fn historical_v1_v2_saved_runs_replay_with_zero_calls() {
    // These are complete mock runs made by the base b61fb56e binary before
    // the v3 implementation. Replay must reconstruct their old packets.
    for version in [1, 2] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/context-v{version}-saved"));
        let report = replay(&path).unwrap();
        assert_eq!(report["verified"], true);
        assert_eq!(report["provider_calls"], 0);
        assert_eq!(report["complete"], true);
    }
}

#[tokio::test]
async fn v3_codex_diagnostics_do_not_apply_unused_jev_choice_size_cap() {
    let (mut manifest, mut oracle) = mixed_inputs();
    for i in 0..5 {
        manifest.cases[1]
            .diagnoses
            .insert(format!("extra{i}"), "x".repeat(1800));
    }
    assert!(
        encoded(&diagnosis_question(&manifest.cases[1]))
            .unwrap()
            .len()
            > 8192
    );
    oracle.manifest_sha256 = manifest.digest().unwrap();
    let plan = preflight(&manifest, &oracle).unwrap();
    assert_eq!(
        plan["cases"][1]["jev_capacity"]["diagnosis"]["model_capacity"],
        "unknown"
    );
    let (provider, jev_seen) = provider(&manifest, false, false);
    let codex_seen = Arc::new(Mutex::new(Vec::new()));
    let diagnostic = codex::Diagnostic::new(
        Coding {
            seen: codex_seen.clone(),
            fail: false,
        },
        coding_profile(),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("codex");
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
    assert_eq!(report["complete"], true);
    assert_eq!(jev_seen.lock().unwrap().len(), 2);
    assert_eq!(codex_seen.lock().unwrap().len(), 4);
    assert_eq!(replay(&output).unwrap()["provider_calls"], 0);
}

#[tokio::test]
async fn bounded_codex_usage_copy_is_reserved_and_replayed() {
    let (manifest, oracle) = large_codex_inputs(7900);
    let plan = preflight(&manifest, &oracle).unwrap();
    let case = &manifest.cases[0];
    let selected = select(&manifest, case, None).unwrap();
    let body = codex::request(
        manifest.diagnostic.as_ref().unwrap(),
        state(&manifest, case, &selected),
        &case.diagnoses,
    )
    .unwrap();
    let body_bytes = encoded(&body).unwrap().len();
    assert!(
        body_bytes > 500_000 && body_bytes <= codex::MAX_REQUEST_BYTES,
        "body_bytes={body_bytes}"
    );
    let (_, usage) = codex::parse(&maximum_bounded_transcript(), &body).unwrap();
    assert!(encoded(&usage).unwrap().len() > 190_000);
    assert_eq!(
        plan["evidence_reservation"]["campaign_limit_bytes"],
        8 * 1024 * 1024
    );

    let (provider, jev_seen) = provider(&manifest, false, false);
    let codex_seen = Arc::new(Mutex::new(Vec::new()));
    let diagnostic = codex::Diagnostic::new(
        BoundedCoding {
            seen: codex_seen.clone(),
        },
        manifest.diagnostic.clone().unwrap(),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("bounded");
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
    assert_eq!(report["complete"], true);
    assert_eq!(jev_seen.lock().unwrap().len(), 2);
    assert_eq!(codex_seen.lock().unwrap().len(), 4);
    let log = std::fs::read_to_string(output.join("calls.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 6);
    let actual = log.lines().next().unwrap().len() + 1;
    assert!(actual > 896 * 1024, "actual={actual}");
    assert!(
        actual
            <= plan["evidence_reservation"]["largest_reserved_call_record_bytes"]
                .as_u64()
                .unwrap() as usize
    );
    assert!(
        actual
            <= plan["evidence_reservation"]["single_record_limit_bytes"]
                .as_u64()
                .unwrap() as usize
    );
    assert_eq!(replay(&output).unwrap()["provider_calls"], 0);
}

#[tokio::test]
async fn whole_campaign_evidence_overflow_refuses_before_any_dispatch() {
    let (mut manifest, mut oracle) = large_codex_inputs(6000);
    let control = preflight(&manifest, &oracle).unwrap();
    assert!(
        control["evidence_reservation"]["reserved_call_record_bytes"]
            .as_u64()
            .unwrap()
            < 8 * 1024 * 1024
    );
    for i in 2..4 {
        let mut case = manifest.cases[i - 2].clone();
        let truth = oracle.cases[&case.id].clone();
        case.id = format!("c{i}");
        oracle.cases.insert(case.id.clone(), truth);
        manifest.cases.push(case);
    }
    manifest.limits.max_calls = 12;
    manifest.limits.max_reserved_usd = 0.04;
    oracle.manifest_sha256 = manifest.digest().unwrap();
    assert_eq!(
        preflight(&manifest, &oracle).unwrap_err().0,
        "context_evidence_campaign_budget"
    );
    let (provider, jev_seen) = provider(&manifest, false, false);
    let codex_seen = Arc::new(Mutex::new(Vec::new()));
    let diagnostic = codex::Diagnostic::new(
        BoundedCoding {
            seen: codex_seen.clone(),
        },
        manifest.diagnostic.clone().unwrap(),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("rejected");
    assert_eq!(
        campaign_with_diagnostic(
            manifest,
            oracle,
            provider,
            diagnostic,
            &output,
            false,
            &CancellationToken::new()
        )
        .await
        .unwrap_err()
        .0,
        "context_evidence_campaign_budget"
    );
    assert!(jev_seen.lock().unwrap().is_empty());
    assert!(codex_seen.lock().unwrap().is_empty());
    assert!(!output.exists());
}

#[tokio::test]
async fn old_manifests_allow_inspection_but_no_new_execution() {
    let (manifest, oracle) = legacy_inputs();
    preflight(&manifest, &oracle).unwrap();
    let (provider, seen) = provider(&manifest, false, false);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("no_new_v1");
    assert_eq!(
        campaign(
            manifest,
            oracle,
            provider,
            &output,
            false,
            &CancellationToken::new()
        )
        .await
        .unwrap_err()
        .0,
        "context_campaign_requires_v3"
    );
    assert!(seen.lock().unwrap().is_empty());
    assert!(!output.exists());
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
        assert_eq!(row["correct"], true);
        assert_eq!(row["missing_essential"], json!([]));
        assert_eq!(row["selected"], json!(["a", "b", "c"]));
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

#[test]
fn declared_essential_count_must_fit_even_when_the_packet_fits_in_bytes() {
    let (manifest, mut oracle) = legacy_inputs();
    oracle.cases.get_mut("c0").unwrap().essential = vec!["a".into(), "b".into()];
    let declared = &oracle.cases["c0"].essential;
    assert_eq!(declared.len(), manifest.limits.selected_chunks.unwrap() + 1);
    assert!(
        encoded(&state(&manifest, &manifest.cases[0], declared))
            .unwrap()
            .len()
            < manifest.limits.context_bytes.unwrap()
    );
    manifest.validate().unwrap();
    oracle.validate(&manifest).unwrap();
    assert_eq!(
        preflight(&manifest, &oracle).unwrap_err().0,
        "context_declared_essential_chunk_limit"
    );
}

#[test]
fn combined_canonical_essential_packet_must_fit_and_exact_boundary_is_accepted() {
    let (mut manifest, mut oracle) = legacy_inputs();
    let case = &mut manifest.cases[0];
    case.chunks[0] = chunk("a", &"é".repeat(200));
    case.chunks[1] = chunk("b", &"é".repeat(200));
    manifest.limits.selected_chunks = Some(2);
    let declared = vec!["b".into(), "a".into()];
    let combined = encoded(&state(&manifest, &manifest.cases[0], &declared))
        .unwrap()
        .len();
    let unescaped = serde_json::to_vec(&state(&manifest, &manifest.cases[0], &declared))
        .unwrap()
        .len();
    manifest.limits.context_bytes = Some(combined - 1);
    assert!(manifest.limits.context_bytes.unwrap() >= 1024);
    assert!(unescaped < manifest.limits.context_bytes.unwrap());
    assert!(
        encoded(&state(&manifest, &manifest.cases[0], &[]))
            .unwrap()
            .len()
            <= manifest.limits.context_bytes.unwrap()
    );
    for id in &declared {
        assert!(
            encoded(&state(
                &manifest,
                &manifest.cases[0],
                std::slice::from_ref(id)
            ))
            .unwrap()
            .len()
                <= manifest.limits.context_bytes.unwrap()
        );
    }
    oracle.cases.get_mut("c0").unwrap().essential = declared;
    oracle.manifest_sha256 = manifest.digest().unwrap();
    manifest.validate().unwrap();
    oracle.validate(&manifest).unwrap();
    assert_eq!(
        preflight(&manifest, &oracle).unwrap_err().0,
        "context_declared_essential_byte_limit"
    );

    // This frozen abstention control can be admitted while its declared packet
    // remains visibly infeasible.
    oracle.cases.get_mut("c0").unwrap().diagnosis = "insufficient".into();
    let control = preflight(&manifest, &oracle).unwrap();
    assert_eq!(control["cases"][0]["declared_essential_count"], 2);
    assert_eq!(
        control["cases"][0]["declared_essential_context_bytes"],
        combined
    );
    assert_eq!(control["cases"][0]["declared_essential_feasible"], false);
    assert_eq!(control["cases"][0]["expected_insufficient_control"], true);

    // The exact byte and count ceilings both admit the non-abstention case.
    manifest.limits.context_bytes = Some(combined);
    oracle.manifest_sha256 = manifest.digest().unwrap();
    oracle.cases.get_mut("c0").unwrap().diagnosis = "right".into();
    let admitted = preflight(&manifest, &oracle).unwrap();
    assert_eq!(admitted["cases"][0]["declared_essential_count"], 2);
    assert_eq!(
        admitted["cases"][0]["declared_essential_context_bytes"],
        combined
    );
    assert_eq!(admitted["cases"][0]["declared_essential_feasible"], true);
    assert_eq!(admitted["cases"][0]["expected_insufficient_control"], false);
}

#[test]
fn a_feasible_declaration_is_admitted_even_when_baseline_omits_it() {
    let (manifest, oracle) = legacy_inputs();
    let admitted = preflight(&manifest, &oracle).unwrap();
    assert_eq!(admitted["cases"][0]["baseline_selected"], json!(["a"]));
    assert_eq!(admitted["cases"][0]["declared_essential_count"], 1);
    assert_eq!(admitted["cases"][0]["declared_essential_feasible"], true);
}

#[test]
fn legacy_declared_essential_packet_is_rejected_on_inspection() {
    let (manifest, mut oracle) = legacy_inputs();
    oracle.cases.get_mut("c0").unwrap().essential = vec!["a".into(), "b".into()];
    assert_eq!(
        preflight(&manifest, &oracle).unwrap_err().0,
        "context_declared_essential_chunk_limit"
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
    too_small.limits.context_bytes = Some(1024);
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
    changed["analysis"]["rows"][0]["correct"] = json!(false);
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
    for field in ["capacity_policy", "evidence_reservation", "capacity_cases"] {
        let mut changed = decode(&original).unwrap();
        changed[field] = json!({"tampered":true});
        std::fs::write(&report_path, encoded(&changed).unwrap()).unwrap();
        assert_eq!(
            replay(&output).unwrap_err().0,
            "context_replay_capacity",
            "{field}"
        );
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
