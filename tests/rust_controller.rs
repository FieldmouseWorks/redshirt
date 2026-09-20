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
            ok: s.actual == s.expected,
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
