use async_trait::async_trait;
use redshirt::{
    evidence::{decode, encoded, load, sha256, view_digest},
    *,
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct State {
    actual: u32,
    expected: u32,
    revision: u32,
    finalized: bool,
    closed: bool,
    selected: u32,
}
struct Fixture {
    state: Arc<Mutex<State>>,
    mode: &'static str,
}
impl Fixture {
    fn new(mode: &'static str) -> Self {
        Self {
            state: Arc::default(),
            mode,
        }
    }
    fn descriptor(&self) -> Descriptor {
        let changed = self.state.lock().unwrap().selected > 0;
        Descriptor {
            identity: json!({"adapter":"synthetic-v1", "build":if self.mode == "identity" && changed {"other"}else{"one"}}),
            setup_mode: if self.mode == "attach" || self.mode == "capability" && changed {
                "attach"
            } else {
                "reset"
            }
            .into(),
        }
    }
}
#[async_trait]
impl Adapter for Fixture {
    async fn describe(&mut self) -> Result<Descriptor> {
        Ok(self.descriptor())
    }
    async fn reset(&mut self) -> Result<()> {
        if self.mode == "reset_error" {
            return Err("reset_error".into());
        }
        Ok(())
    }
    async fn observe(&mut self) -> Result<Frame> {
        let s = self.state.lock().unwrap();
        let changed = s.selected > 0;
        Ok(Frame {
            observation: Observation {
                environment: "fixture".into(),
                epoch: "one".into(),
                guard: format!(
                    "{}",
                    s.revision + u32::from(self.mode == "stale" && changed)
                ),
                view: if self.mode == "large" {
                    json!({"visible":"x".repeat(4096)})
                } else {
                    json!({"counter":s.actual})
                },
                ready: self.mode != "busy",
                terminal: if self.mode == "death" {
                    Some("death".into())
                } else if self.mode.starts_with("terminal") && s.actual >= 1 {
                    Some("complete".into())
                } else {
                    None
                },
            },
            candidates: vec![Candidate {
                id: "increment".into(),
                description: "Press increment".into(),
                operation: json!({"button":if self.mode == "candidate" && changed {"replaced"}else{"increment"}}),
                inputs: 1,
                needs_ready: true,
            }],
        })
    }
    async fn verify(&mut self) -> Result<Descriptor> {
        Ok(self.descriptor())
    }
    async fn execute(&mut self, _: &Value) -> Result<Value> {
        let mut s = self.state.lock().unwrap();
        s.expected += 1;
        s.revision += 1;
        if self.mode != "omitted" {
            s.actual += 1;
        }
        if self.mode == "uncertain" {
            return Err("lost_receipt".into());
        }
        Ok(json!({"claimed":"success"}))
    }
    async fn evaluate(&mut self, phase: &str, _: Option<&Value>) -> Result<Verdict> {
        let mut s = self.state.lock().unwrap();
        if phase == "final" {
            s.finalized = true;
        }
        Ok(Verdict {
            ok: s.actual == s.expected && !(phase == "final" && self.mode == "terminal_bad_final"),
            progress: phase == "after" && self.mode != "stationary",
            checks: json!({"actual":s.actual,"expected":s.expected}),
        })
    }
    async fn close(&mut self) -> Result<()> {
        self.state.lock().unwrap().closed = true;
        Ok(())
    }
}
struct Selector {
    state: Arc<Mutex<State>>,
    mode: &'static str,
    cancel: CancellationToken,
}
#[async_trait]
impl Provider for Selector {
    fn name(&self) -> &str {
        "synthetic"
    }
    async fn select(&mut self, request: Value) -> Result<String> {
        assert_eq!(request.as_object().unwrap().len(), 4);
        assert!(request.get("identity").is_none() && request.get("guard").is_none());
        self.state.lock().unwrap().selected += 1;
        if self.mode == "cancel" {
            self.cancel.cancel();
        }
        if self.mode == "slow" || self.mode == "cancel" {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        Ok(match self.mode {
            "stop" => "stop",
            "unknown" => "absent",
            _ => "increment",
        }
        .into())
    }
}

async fn exercise(mode: &'static str, limits: Limits) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let mut env = Fixture::new(mode);
    let cancel = CancellationToken::new();
    let mut provider = Selector {
        state: env.state.clone(),
        mode,
        cancel: cancel.clone(),
    };
    let result = run(
        &mut env,
        Some(&mut provider),
        None,
        &dir.path().join("run"),
        limits,
        &cancel,
    )
    .await
    .unwrap();
    let s = env.state.lock().unwrap();
    assert!(s.finalized && s.closed, "{mode}");
    result
}
#[tokio::test]
async fn refusal_and_negative_controls() {
    for (mode, stop, inputs) in [
        ("stop", "selector_stop", 0),
        ("unknown", "unknown_candidate", 0),
        ("busy", "busy_refused", 0),
        ("stale", "stale_observation", 0),
        ("candidate", "candidate_changed", 0),
        ("identity", "identity_changed", 0),
        ("capability", "identity_changed", 0),
        ("death", "death", 0),
        ("large", "observation_size", 0),
        ("omitted", "evaluation_failed", 1),
        ("uncertain", "lost_receipt", 1),
        ("reset_error", "reset_error", 0),
        ("cancel", "cancelled", 0),
    ] {
        let report = exercise(mode, Limits::default()).await;
        assert_eq!(report["stop"], stop, "{mode}");
        assert_eq!(report["attempted_inputs"], inputs, "{mode}");
        if ["omitted", "uncertain", "capability"].contains(&mode) {
            assert_eq!(report["replayable"], false);
        }
    }
}
#[tokio::test]
async fn limits_stop_and_finalize() {
    for (mode, limits, stop, inputs) in [
        (
            "normal",
            Limits {
                inputs: 2,
                ..Limits::default()
            },
            "input_budget",
            2,
        ),
        (
            "normal",
            Limits {
                requests: 2,
                ..Limits::default()
            },
            "request_budget",
            2,
        ),
        ("stationary", Limits::default(), "no_progress", 3),
        (
            "slow",
            Limits {
                operation_seconds: 0.02,
                ..Limits::default()
            },
            "operation_timeout",
            0,
        ),
    ] {
        let report = exercise(mode, limits).await;
        assert_eq!(report["stop"], stop);
        assert_eq!(report["attempted_inputs"], inputs);
    }
}
#[tokio::test]
async fn concrete_replay_identity_preconditions_and_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("first");
    let cancel = CancellationToken::new();
    let mut env = Fixture::new("normal");
    let mut script = Scripted(vec!["increment".into(), "increment".into(), "stop".into()].into());
    let first = run(
        &mut env,
        Some(&mut script),
        None,
        &output,
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    let saved = load(&output.join("replay.json"), 65536).unwrap();
    let replay = run(
        &mut Fixture::new("normal"),
        None,
        Some(saved.clone()),
        &dir.path().join("replay"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(replay["replay_complete"], true);
    assert_eq!(replay["requests"], 0);
    assert_eq!(first["evaluations"], replay["evaluations"]);
    let manifest = load(&output.join("artifacts.json"), 65536).unwrap();
    for (name, info) in manifest.as_object().unwrap() {
        let bytes = std::fs::read(output.join(name)).unwrap();
        assert_eq!(info["bytes"], bytes.len());
        assert_eq!(info["sha256"], sha256(&bytes));
    }
    for (i, reason) in [
        "replay_identity_or_shape",
        "replay_precondition",
        "replay_candidate_unavailable",
        "replay_mismatch",
    ]
    .iter()
    .enumerate()
    {
        let mut forged = saved.clone();
        match i {
            0 => forged["identity"] = json!({}),
            1 => forged["steps"][0]["view"] = "wrong".into(),
            2 => forged["steps"][0]["operation"] = json!({"button":"elsewhere"}),
            _ => forged["steps"][0]["verdict"]["ok"] = false.into(),
        }
        let result = run(
            &mut Fixture::new("normal"),
            None,
            Some(forged),
            &dir.path().join(format!("bad{i}")),
            Limits::default(),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(result["stop"], *reason);
        assert_eq!(result["replay_complete"], false);
    }
}
#[tokio::test]
async fn terminal_replay_completes_only_after_every_checked_step() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let output = dir.path().join("first");
    let mut script = Scripted(vec!["increment".into()].into());
    let first = run(
        &mut Fixture::new("terminal"),
        Some(&mut script),
        None,
        &output,
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(first["stop"], "complete");
    let saved = load(&output.join("replay.json"), 65536).unwrap();
    let replay = run(
        &mut Fixture::new("terminal"),
        None,
        Some(saved.clone()),
        &dir.path().join("replay"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(replay["replay_complete"], true);
    assert_eq!(replay["requests"], 0);
    assert_eq!(replay["evaluations"], first["evaluations"]);
    assert_eq!(replay["cleanup"], true);

    let bad_final = run(
        &mut Fixture::new("terminal_bad_final"),
        None,
        Some(saved),
        &dir.path().join("bad-final"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(bad_final["replay_complete"], false);
    assert_eq!(bad_final["final"]["ok"], false);
    assert_eq!(bad_final["cleanup"], true);

    let longer = dir.path().join("longer");
    let mut script = Scripted(vec!["increment".into(), "increment".into(), "stop".into()].into());
    run(
        &mut Fixture::new("normal"),
        Some(&mut script),
        None,
        &longer,
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    let saved = load(&longer.join("replay.json"), 65536).unwrap();
    let early = run(
        &mut Fixture::new("terminal"),
        None,
        Some(saved),
        &dir.path().join("early"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(early["stop"], "complete");
    assert_eq!(early["replay_complete"], false);
    assert_eq!(early["attempted_inputs"], 1);
    assert_eq!(early["requests"], 0);
    assert_eq!(early["cleanup"], true);

    let empty = dir.path().join("empty");
    let mut script = Scripted(vec!["stop".into()].into());
    run(
        &mut Fixture::new("normal"),
        Some(&mut script),
        None,
        &empty,
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    let saved = load(&empty.join("replay.json"), 65536).unwrap();
    let initial_terminal = run(
        &mut Fixture::new("death"),
        None,
        Some(saved),
        &dir.path().join("initial-terminal"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(initial_terminal["stop"], "death");
    assert_eq!(initial_terminal["replay_complete"], false);
    assert_eq!(initial_terminal["attempted_inputs"], 0);
    assert_eq!(initial_terminal["requests"], 0);
    assert_eq!(initial_terminal["cleanup"], true);
}

#[tokio::test]
async fn attached_session_never_grants_reset() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let mut script = Scripted(vec!["increment".into(), "stop".into()].into());
    let report = run(
        &mut Fixture::new("attach"),
        Some(&mut script),
        None,
        &dir.path().join("attached"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(report["setup_verified"], true);
    assert_eq!(report["reset_verified"], false);
    assert_eq!(report["replayable"], false);
    let mut saved = load(&dir.path().join("attached/replay.json"), 65536).unwrap();
    saved["complete"] = true.into();
    let report = run(
        &mut Fixture::new("attach"),
        None,
        Some(saved),
        &dir.path().join("forged"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(report["stop"], "replay_unavailable");
    assert_eq!(report["setup_verified"], false);
}
#[test]
fn strict_json_and_portable_view_profile() {
    for raw in [
        br#"{"a":1,"a":2}"#.as_slice(),
        br#"{"a":{"b":1,"b":2}}"#,
        b"NaN",
    ] {
        assert!(decode(raw).is_err());
    }
    assert_eq!(
        String::from_utf8(encoded(&json!({"z":"é😀","a":1})).unwrap()).unwrap(),
        r#"{"a":1,"z":"\u00e9\ud83d\ude00"}"#
    );
    // The integer-only restriction is superseded by the Python numeric oracle.
    assert_eq!(
        view_digest(&json!({"clock":0.2})).unwrap(),
        sha256(b"{\"clock\":0.2}")
    );
    assert!(
        Limits {
            captures: 1,
            ..Limits::default()
        }
        .validate()
        .is_err()
    );
}

#[tokio::test]
async fn stop_still_detects_changed_setup_authority() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let mut env = Fixture::new("capability");
    let mut selector = Selector {
        state: env.state.clone(),
        mode: "stop",
        cancel: cancel.clone(),
    };
    let report = run(
        &mut env,
        Some(&mut selector),
        None,
        &dir.path().join("run"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(report["replayable"], false);
    assert_eq!(report["stop"], "identity_changed");
    let state = env.state.lock().unwrap();
    assert!(state.finalized && state.closed);
}

#[test]
fn ascii_delete_matches_python_v1_escape() {
    assert_eq!(
        encoded(&json!({"label":"\u{7f}"})).unwrap(),
        br#"{"label":"\u007f"}"#
    );
}

struct BatchTransport {
    state: Arc<Mutex<State>>,
    confidence: f64,
    cancel: Option<CancellationToken>,
}
#[async_trait]
impl jev::Transport for BatchTransport {
    async fn post(&mut self, body: Vec<u8>) -> Result<(u16, Vec<u8>)> {
        let request = decode(&body)?;
        assert_eq!(request["questions"].as_object().unwrap().len(), 3);
        assert_eq!(request["state"].as_object().unwrap().len(), 4);
        assert_eq!(
            request["state"]["observation"].as_object().unwrap().len(),
            1
        );
        assert_eq!(
            request["questions"]["action"]["criteria"],
            request["state"]["candidates"]
        );
        self.state.lock().unwrap().selected += 1;
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
            std::future::pending::<()>().await;
        }
        let done = request["state"]["observation"]["counter"] == 1;
        let reply = json!({"model":jev::MODEL,"answers":{
            "action":{"type":"choice","choice":if done {"stop"} else {"increment"},
                "probabilities":{"increment":if done {0.1} else {0.9},"stop":if done {0.9} else {0.1}},
                "confidence":self.confidence},
            "visible":{"type":"noul","noul":1.0},
            "urgency":{"type":"score","score":0.0,"legend":{"0":"Routine","1":"Urgent"},
                "probabilities":{"0":1.0,"1":0.0},"confidence":1.0}
        },"usage":{"input_tokens":180,"output_tokens":50}});
        Ok((200, encoded(&reply)?))
    }
}
fn batch_config() -> jev::Config {
    serde_json::from_value(json!({"questions":{
        "visible":{"type":"noul","instructions":"Is the counter visible?"},
        "urgency":{"type":"score","instructions":"How urgent is this task?","criteria":["Routine","Urgent"]}
    },"policy":{"min_confidence":0.8}})).unwrap()
}

#[tokio::test]
async fn batched_selection_matches_scripted_outcomes_and_replays_without_provider() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let mut env = Fixture::new("normal");
    let mut provider = jev::Jev::new(
        BatchTransport {
            state: env.state.clone(),
            confidence: 0.9,
            cancel: None,
        },
        batch_config(),
    )
    .unwrap();
    let output = dir.path().join("batch");
    let batch = run(
        &mut env,
        Some(&mut provider),
        None,
        &output,
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    let mut script = Scripted(vec!["increment".into(), "stop".into()].into());
    let baseline = run(
        &mut Fixture::new("normal"),
        Some(&mut script),
        None,
        &dir.path().join("baseline"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(batch["stop"], "selector_stop");
    assert_eq!(batch["operations"], baseline["operations"]);
    assert_eq!(batch["evaluations"], baseline["evaluations"]);
    assert_eq!(batch["requests"], 2);
    assert_eq!(batch["attempted_inputs"], 1);
    assert_eq!(provider.calls(), 2);
    let events = std::fs::read_to_string(output.join("events.jsonl")).unwrap();
    let receipts: Vec<Value> = events
        .lines()
        .map(|line| decode(line.as_bytes()).unwrap())
        .filter(|v| v["event"] == "provider_receipt")
        .collect();
    assert_eq!(receipts.len(), 2);
    let saved = load(&output.join("replay.json"), 65536).unwrap();
    let replay = run(
        &mut Fixture::new("normal"),
        None,
        Some(saved),
        &dir.path().join("replay"),
        Limits::default(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(replay["replay_complete"], true);
    assert_eq!(replay["requests"], 0);
    assert_eq!(provider.calls(), 2);
    assert_eq!(replay["evaluations"], batch["evaluations"]);
}

#[tokio::test]
async fn batched_provider_preserves_controller_refusals_and_finalization() {
    for (mode, confidence, stopped, expected_inputs) in [
        ("normal", 0.7, "provider_uncertain", 0),
        ("stale", 0.9, "stale_observation", 0),
        ("busy", 0.9, "busy_refused", 0),
        ("omitted", 0.9, "evaluation_failed", 1),
        ("cancel", 0.9, "cancelled", 0),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let mut env = Fixture::new(mode);
        let mut provider = jev::Jev::new(
            BatchTransport {
                state: env.state.clone(),
                confidence,
                cancel: if mode == "cancel" {
                    Some(cancel.clone())
                } else {
                    None
                },
            },
            batch_config(),
        )
        .unwrap();
        let output = dir.path().join("run");
        let report = run(
            &mut env,
            Some(&mut provider),
            None,
            &output,
            Limits::default(),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(report["stop"], stopped, "{mode}");
        assert_eq!(report["attempted_inputs"], expected_inputs, "{mode}");
        assert_eq!(provider.calls(), 1);
        let state = env.state.lock().unwrap();
        assert!(state.finalized && state.closed);
        let events = std::fs::read_to_string(output.join("events.jsonl")).unwrap();
        let receipt: Value = events
            .lines()
            .map(|line| decode(line.as_bytes()).unwrap())
            .find(|v| v["event"] == "provider_receipt")
            .unwrap();
        let outcome = receipt["data"]["outcome"].as_str().unwrap();
        assert_eq!(
            outcome,
            match mode {
                "normal" => "abstained",
                "cancel" => "interrupted",
                _ => "accepted",
            }
        );
    }
}

#[tokio::test]
async fn batched_evidence_reservation_refuses_before_model_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let mut env = Fixture::new("normal");
    let mut provider = jev::Jev::new(
        BatchTransport {
            state: env.state.clone(),
            confidence: 0.9,
            cancel: None,
        },
        batch_config(),
    )
    .unwrap();
    let report = run(
        &mut env,
        Some(&mut provider),
        None,
        &dir.path().join("run"),
        Limits {
            evidence_bytes: 262144,
            ..Limits::default()
        },
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(report["stop"], "evidence_budget");
    assert_eq!(report["requests"], 0);
    assert_eq!(provider.calls(), 0);
    assert_eq!(report["attempted_inputs"], 0);
    let state = env.state.lock().unwrap();
    assert!(state.finalized && state.closed);
}
