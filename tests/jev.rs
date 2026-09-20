use async_trait::async_trait;
use redshirt::{
    decision::{Answer, ConfidencePolicy, Question},
    evidence::{decode, encoded, sha256},
    jev::{Config, Jev, MAX_BYTES, MODEL, Transport},
    *,
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn request() -> Value {
    json!({"version":1,"observation":{"objective":"Increment once."},
        "candidates":{"increment":"Increment.","stop":"Stop."},"remaining_inputs":24})
}
fn config() -> Config {
    serde_json::from_value(json!({"version":1,"request_limit":2,"questions":{
        "visible":{"type":"noul","instructions":"Is the counter visible?"},
        "urgency":{"type":"score","instructions":"How urgent is this task?",
            "criteria":["Routine","Urgent"]},
        "category":{"type":"choice","instructions":"What kind of task?",
            "criteria":{"interaction":"Input task","reading":"Reading task"}}
    }}))
    .unwrap()
}
fn response() -> Value {
    json!({"model":MODEL,"answers":{
        "action":{"type":"choice","choice":"increment","confidence":0.7,
            "probabilities":{"increment":0.8,"stop":0.2}},
        "visible":{"type":"noul","noul":0.98},
        "urgency":{"type":"score","score":0.25,"confidence":0.6,
            "legend":{"0":"Routine","1":"Urgent"},"probabilities":{"0":0.75,"1":0.25}},
        "category":{"type":"choice","choice":"interaction","confidence":0.9,
            "probabilities":{"interaction":0.95,"reading":0.05}}
    },"usage":{"input_tokens":300,"output_tokens":80}})
}
struct Stub {
    seen: Arc<Mutex<Vec<Vec<u8>>>>,
    status: u16,
    raw: Vec<u8>,
    pending: bool,
}
#[async_trait]
impl Transport for Stub {
    async fn post(&mut self, body: Vec<u8>) -> Result<(u16, Vec<u8>)> {
        self.seen.lock().unwrap().push(body);
        if self.pending {
            std::future::pending::<()>().await;
        }
        Ok((self.status, self.raw.clone()))
    }
}
fn stub(raw: Vec<u8>) -> Stub {
    Stub {
        seen: Arc::default(),
        status: 200,
        raw,
        pending: false,
    }
}

#[tokio::test]
async fn one_batch_preserves_state_menu_answers_and_usage() {
    let transport = stub(encoded(&response()).unwrap());
    let seen = transport.seen.clone();
    let mut provider = Jev::new(transport, config()).unwrap();
    assert_eq!(provider.select(request()).await.unwrap(), "increment");
    let receipt = provider.take_evidence().pop().unwrap();
    let calls = seen.lock().unwrap();
    assert_eq!(calls.len(), 1);
    let body = decode(&calls[0]).unwrap();
    assert_eq!(body["state"], request());
    assert_eq!(
        body["questions"]["action"]["criteria"],
        request()["candidates"]
    );
    assert_eq!(body["questions"].as_object().unwrap().len(), 4);
    assert_eq!(receipt["request_sha256"], sha256(&calls[0]));
    assert_eq!(receipt["usage"]["input_tokens"], 300);
    assert!(receipt["billed_usd"].is_null());
    assert_eq!(receipt["reserved_input_tokens"], 65536);
    assert_eq!(
        decode(receipt["response"].as_str().unwrap().as_bytes()).unwrap(),
        response()
    );
    assert_eq!(receipt["policy"]["disposition"], "accept");
    assert!(encoded(&receipt).unwrap().len() < provider.evidence_limit());
}

#[tokio::test]
async fn full_menu_and_approximate_totals_are_not_pruned_or_normalized() {
    let mut state = request();
    let candidates = state["candidates"].as_object_mut().unwrap();
    candidates.remove("increment");
    for i in 0..96 {
        candidates.insert(i.to_string(), format!("Visible target {i}").into());
    }
    let probabilities: serde_json::Map<_, _> = candidates
        .keys()
        .map(|k| (k.clone(), if k == "73" { json!(0.998) } else { json!(0.0) }))
        .collect();
    let reply = json!({"model":MODEL,"answers":{"action":{"type":"choice","choice":"73",
        "probabilities":probabilities,"confidence":0.99}},"usage":{"input_tokens":200,"output_tokens":20}});
    let transport = stub(encoded(&reply).unwrap());
    let seen = transport.seen.clone();
    let mut provider = Jev::new(transport, Config::default()).unwrap();
    assert_eq!(provider.select(state.clone()).await.unwrap(), "73");
    let body = decode(&seen.lock().unwrap()[0]).unwrap();
    assert_eq!(body["questions"]["action"]["criteria"], state["candidates"]);
    let receipt = provider.take_evidence().pop().unwrap();
    assert_eq!(
        receipt["probability_policy"]["action"]["classification"],
        "accepted_approximate"
    );
    assert_eq!(receipt["probability_policy"]["action"]["normalized"], false);
    assert_eq!(
        decode(receipt["response"].as_str().unwrap().as_bytes()).unwrap(),
        reply
    );
}

#[tokio::test]
async fn malformed_auxiliary_answers_refuse_the_whole_batch() {
    for case in 0..15 {
        let mut reply = response();
        match case {
            0 => {
                reply["answers"].as_object_mut().unwrap().remove("visible");
            }
            1 => {
                reply["answers"]["extra"] = json!({"type":"noul","noul":0.9});
            }
            2 => {
                reply["answers"]["visible"] = json!({"type":"choice","choice":"yes","confidence":1.0,"probabilities":{"yes":1.0}})
            }
            3 => reply["answers"]["visible"]["noul"] = json!(1.1),
            4 => reply["answers"]["urgency"]["score"] = json!(2.0),
            5 => reply["answers"]["urgency"]["legend"]["1"] = json!("Replaced"),
            6 => reply["answers"]["urgency"]["probabilities"] = json!({"0":0.75,"8":0.25}),
            7 => reply["answers"]["urgency"]["score"] = json!(0.9),
            8 => reply["answers"]["category"]["choice"] = json!("reading"),
            9 => reply["answers"]["action"]["probabilities"] = json!({"increment":0.9}),
            10 => reply["answers"]["action"]["confidence"] = json!(true),
            11 => reply["model"] = json!("jev-latest"),
            12 => reply["usage"]["input_tokens"] = json!(65537),
            13 => reply["answers"]["visible"]["noul"] = json!(null),
            _ => {
                reply["answers"]["category"]["probabilities"] =
                    json!({"interaction":0.7,"reading":0.1})
            }
        }
        let mut provider = Jev::new(stub(encoded(&reply).unwrap()), config()).unwrap();
        assert!(provider.select(request()).await.is_err(), "case {case}");
        assert_eq!(provider.calls(), 1);
        assert_eq!(provider.take_evidence()[0]["outcome"], "failed");
    }
    for raw in [
        br#"{"model":"a","model":"b"}"#.to_vec(),
        b"NaN".to_vec(),
        vec![b'x'; MAX_BYTES + 1],
    ] {
        let mut provider = Jev::new(stub(raw), config()).unwrap();
        assert!(provider.select(request()).await.is_err());
        assert_eq!(provider.take_evidence()[0]["outcome"], "failed");
    }
}

#[tokio::test]
async fn uncertainty_is_explicit_and_the_threshold_is_opt_in() {
    for (minimum, accepted) in [(None, true), (Some(0.7), true), (Some(0.71), false)] {
        let mut settings = config();
        settings.policy.min_confidence = minimum;
        let mut provider = Jev::new(stub(encoded(&response()).unwrap()), settings).unwrap();
        let result = provider.select(request()).await;
        if accepted {
            assert_eq!(result.unwrap(), "increment");
        } else {
            assert_eq!(result.unwrap_err().0, "provider_uncertain");
        }
        let receipt = provider.take_evidence().pop().unwrap();
        assert_eq!(receipt["policy"]["action"], "increment");
        assert_eq!(receipt["policy"]["confidence"], 0.7);
        assert_eq!(receipt["policy"]["minimum"], json!(minimum));
        assert_eq!(
            receipt["outcome"],
            if accepted { "accepted" } else { "abstained" }
        );
    }
}

#[tokio::test]
async fn dropped_selection_retains_reservation_and_cannot_retry_implicitly() {
    let mut transport = stub(vec![]);
    transport.pending = true;
    let seen = transport.seen.clone();
    let mut provider = Jev::new(
        transport,
        Config {
            request_limit: 1,
            ..Config::default()
        },
    )
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), provider.select(request()))
            .await
            .is_err()
    );
    assert_eq!(
        provider.select(request()).await.unwrap_err().0,
        "provider_receipt_pending"
    );
    let receipt = provider.take_evidence().pop().unwrap();
    assert_eq!(receipt["outcome"], "interrupted");
    assert_eq!(receipt["reserved_input_tokens"], 65536);
    assert_eq!(
        provider.select(request()).await.unwrap_err().0,
        "provider_request_budget"
    );
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert!(provider.take_evidence().is_empty());
}

#[tokio::test]
async fn absolute_deadline_and_http_error_preserve_failed_receipts() {
    let mut transport = stub(vec![]);
    transport.pending = true;
    let mut provider = Jev::new(transport, Config::default()).unwrap();
    assert_eq!(
        provider.select(request()).await.unwrap_err().0,
        "provider_timeout"
    );
    assert_eq!(provider.take_evidence()[0]["outcome"], "failed");
    let mut transport = stub(b"overloaded".to_vec());
    transport.status = 529;
    let seen = transport.seen.clone();
    let mut provider = Jev::new(transport, Config::default()).unwrap();
    assert_eq!(
        provider.select(request()).await.unwrap_err().0,
        "provider_http_status"
    );
    let receipt = provider.take_evidence().pop().unwrap();
    assert_eq!(receipt["http_status"], 529);
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn configuration_and_probability_boundaries() {
    for minimum in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
        let policy = ConfidencePolicy {
            min_confidence: Some(minimum),
        };
        assert!(policy.validate().is_err());
    }
    for settings in [
        json!({"version":2}),
        json!({"request_limit":0}),
        json!({"request_limit":13}),
        json!({"questions":{"action":{"type":"noul","instructions":"Override"}}}),
        json!({"questions":{"s":{"type":"score","instructions":"Rate","criteria":["One"]}}}),
    ] {
        let config: Config = serde_json::from_value(settings).unwrap();
        assert!(config.validate().is_err());
    }
    let mut settings = Config::default();
    for i in 0..8 {
        settings.questions.insert(
            i.to_string(),
            Question::Noul {
                instructions: "Visible?".into(),
            },
        );
    }
    assert!(settings.validate().is_err());
    let question = Question::Noul {
        instructions: "Visible?".into(),
    };
    assert!(
        Answer::Noul { noul: f64::NAN }
            .validate_for(&question)
            .is_err()
    );
    assert!(serde_json::from_value::<Config>(json!({"commands":["arbitrary"]})).is_err());
}

#[tokio::test]
async fn oversized_batch_refuses_before_dispatch() {
    let mut state = request();
    state["observation"]["visible"] = json!("x".repeat(3800));
    let candidates = state["candidates"].as_object_mut().unwrap();
    for i in 0..20 {
        candidates.insert(i.to_string(), json!("x".repeat(320)));
    }
    let transport = stub(vec![]);
    let seen = transport.seen.clone();
    let mut provider = Jev::new(transport, config()).unwrap();
    assert_eq!(
        provider.select(state).await.unwrap_err().0,
        "provider_request_size"
    );
    assert_eq!(provider.calls(), 0);
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn cli_rejects_conflicting_modes_and_missing_transport_before_adapter_setup() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("jev.json");
    std::fs::write(&config_path, b"{}").unwrap();
    let output_path = dir.path().join("output");
    let invoke = |conflict: bool| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_redshirt"));
        command
            .env_remove("TYPESAFE_API_KEY")
            .arg("--output")
            .arg(&output_path)
            .arg("--jev")
            .arg(&config_path);
        if conflict {
            command.args(["--script", "absent"]);
        }
        command.args(["--adapter", "must-not-launch-this-command"]);
        command.output().unwrap()
    };
    let result = invoke(true);
    assert_eq!(result.status.code(), Some(2));
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("choose_one_mode")
    );
    let result = invoke(false);
    assert_eq!(result.status.code(), Some(2));
    let expected = if cfg!(feature = "jev-http") {
        "missing_or_invalid_key"
    } else {
        "jev_http_feature_required"
    };
    assert!(String::from_utf8(result.stderr).unwrap().contains(expected));
    assert!(!output_path.exists());
}
