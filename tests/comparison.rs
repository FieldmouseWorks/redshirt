use async_trait::async_trait;
use redshirt::{comparison::*, evidence::load, *};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn manifest() -> Manifest {
    let adapter = vec![
        "python3".into(),
        format!("{}/tests/comparison_fixture.py", env!("CARGO_MANIFEST_DIR")),
    ];
    Manifest {
        version: 1,
        cases: vec![
            Case {
                id: "first".into(),
                split: Split::Calibration,
                adapter: adapter.clone(),
            },
            Case {
                id: "second".into(),
                split: Split::HeldOut,
                adapter,
            },
        ],
        limits: Limits {
            requests: 2,
            ..Limits::default()
        },
        jev: jev::Config {
            request_limit: 2,
            ..jev::Config::default()
        },
        max_live_calls: 4,
        max_reserved_usd: 0.02,
        thresholds: vec![0.0, 0.7, 0.9],
    }
}

#[test]
fn admission_checks_whole_campaign_before_any_dispatch() {
    let good = manifest();
    good.validate().unwrap();
    assert!((good.reservation() - 0.011010048).abs() < 1e-12);
    let mut bad = good.clone();
    bad.max_live_calls = 3;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.max_reserved_usd = 0.01;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.cases[1].split = Split::Calibration;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.cases[1].id = "../escape".into();
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.jev.policy.min_confidence = Some(0.7);
    assert!(bad.validate().is_err());
    let mut bad = good;
    bad.limits.requests = 3;
    assert!(bad.validate().is_err());
}

#[test]
fn independent_task_completion_is_not_a_clean_invariant_check() {
    let mut report = json!({"setup_verified":true,"identity_stable":true,"cleanup":true,"stop":"selector_stop",
        "operations":[],"evaluations":[],"final":{"ok":true,"checks":{}}});
    let receipt = json!({"event":"provider_receipt","data":{"model":jev::MODEL,
        "reserved_usd":0.002752512,"policy":{"confidence":0.8}}});
    let events = vec![
        receipt,
        json!({"event":"decision","data":{"candidate_id":"stop"}}),
    ];
    let m = measure(&report, &events, true);
    assert!(!m.valid && !m.task_complete);
    assert_eq!(m.unknown_usage_calls, 1);
    assert_eq!(m.decisions[0].useful, None);
    report["final"]["checks"]["comparison"] = json!({"complete":false,"useful_action":null});
    let m = measure(&report, &events, false);
    assert!(m.valid && !m.task_complete);
    assert_eq!(m.decisions[0].useful, Some(false));
    report["final"]["checks"]["comparison"]["complete"] = true.into();
    assert!(measure(&report, &events, false).task_complete);
    report["final"]["ok"] = false.into();
    assert!(!measure(&report, &events, false).task_complete);
    report["final"]["ok"] = true.into();
    report["operations"] = json!([{}]);
    report["evaluations"] =
        json!([{"ok":true,"checks":{"comparison":{"complete":true,"useful_action":"yes"}}}]);
    let m = measure(&report, &events, false);
    assert!(!m.valid && !m.task_complete);
    assert_eq!(m.ungraded_attempts, 1);
}

#[test]
fn thresholds_keep_held_out_cases_and_unknown_effects_separate() {
    let rows = vec![
        (
            Split::Calibration,
            vec![
                Decision {
                    confidence: Some(0.6),
                    useful: Some(false),
                },
                Decision {
                    confidence: Some(0.8),
                    useful: Some(true),
                },
            ],
        ),
        (
            Split::HeldOut,
            vec![
                Decision {
                    confidence: Some(0.95),
                    useful: Some(false),
                },
                Decision {
                    confidence: Some(0.8),
                    useful: None,
                },
            ],
        ),
    ];
    let calibration = thresholds(&rows, Split::Calibration, &[0.0, 0.7]);
    assert_eq!(calibration[0]["accepted_mistakes"], 1);
    assert_eq!(calibration[1]["accepted_mistakes"], 0);
    assert_eq!(calibration[1]["abstained"], 1);
    let held = thresholds(&rows, Split::HeldOut, &[0.7]);
    assert_eq!(held[0]["accepted_mistakes"], 1);
    assert_eq!(held[0]["ungraded"], 1);
}

struct Counted(Arc<AtomicUsize>, bool);
#[async_trait]
impl Provider for Counted {
    fn name(&self) -> &str {
        "counter"
    }
    async fn select(&mut self, _: Value) -> Result<String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        if self.1 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok("stop".into())
    }
}
#[tokio::test]
async fn changed_initial_request_refuses_before_provider_and_cancel_retains_timing() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut p = MatchedProvider::new(
        Box::new(Counted(calls.clone(), false)),
        Some("different".into()),
    );
    assert_eq!(
        p.select(json!({})).await.unwrap_err().0,
        "comparison_initial_mismatch"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut p = MatchedProvider::new(Box::new(Counted(calls.clone(), true)), None);
    assert!(
        tokio::time::timeout(Duration::from_millis(5), p.select(json!({})))
            .await
            .is_err()
    );
    p.take_evidence();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(p.elapsed_ms.len(), 1);
}

#[tokio::test]
async fn mock_campaign_and_concrete_replay_use_the_existing_rust_controller() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("campaign");
    let mut plan = manifest();
    plan.jev.questions = serde_json::from_value(json!({
        "status":{"type":"choice","instructions":"Is the counter complete?","criteria":{"yes":"Complete","no":"Incomplete"}},
        "score":{"type":"score","instructions":"Score progress","criteria":["None","Complete"]},
        "value":{"type":"noul","instructions":"Give a number"}
    })).unwrap();
    let report = campaign(plan, &output, Mode::Mock, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(report["reserved_calls"], 0);
    let runs = report["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 4);
    assert!(
        runs.iter()
            .all(|r| r["measurement"]["task_complete"] == true)
    );
    assert_eq!(
        runs[0]["initial_request_sha256"],
        runs[2]["initial_request_sha256"]
    );
    let replay = output.join("first-jev-mock/replay.json");
    let destination = temp.path().join("replay");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_redshirt"))
        .args([
            "--output",
            destination.to_str().unwrap(),
            "--replay",
            replay.to_str().unwrap(),
            "--adapter",
            "python3",
        ])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/comparison_fixture.py"))
        .status()
        .unwrap();
    assert!(status.success());
    let replay_report = load(&destination.join("report.json"), 65536).unwrap();
    assert_eq!(replay_report["requests"], 0);
    assert_eq!(replay_report["replay_complete"], true);
}
