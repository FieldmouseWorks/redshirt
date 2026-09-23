use async_trait::async_trait;
use redshirt::{
    choice::{Choice, Config, MAX_BYTES, Profile, Reply, Transport},
    comparison::MatchedProvider,
    evidence::{decode, encoded, sha256},
    *,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

fn config(profile: Profile) -> Config {
    Config {
        version: 1,
        profile,
        request_limit: 2,
        request_bytes: MAX_BYTES,
    }
}
fn request() -> Value {
    json!({"version":1,"observation":{"objective":"Increment once."},
        "candidates":{"increment":"Increment.","stop":"Stop."},"remaining_inputs":12})
}
fn response(profile: Profile, content: &str) -> Value {
    json!({"model":profile.model(),"choices":[{"index":0,"finish_reason":"stop",
        "message":{"role":"assistant","content":content,"refusal":null}}],
        "usage":{"prompt_tokens":100,"completion_tokens":10,"total_tokens":110,
            "prompt_tokens_details":{"cached_tokens":20},
            "completion_tokens_details":{"reasoning_tokens":2},
            "prompt_cache_hit_tokens":20,"prompt_cache_miss_tokens":80}})
}
struct Stub {
    seen: Arc<Mutex<Vec<Vec<u8>>>>,
    status: u16,
    raw: Vec<u8>,
    overflow: bool,
    pending: bool,
}
#[async_trait]
impl Transport for Stub {
    async fn post(&mut self, body: Vec<u8>) -> Result<Reply> {
        self.seen.lock().unwrap().push(body);
        if self.pending {
            std::future::pending::<()>().await;
        }
        Ok(Reply {
            status: self.status,
            body: self.raw.clone(),
            overflow: self.overflow,
            rate_limits: BTreeMap::new(),
        })
    }
}
fn stub(raw: Vec<u8>) -> Stub {
    Stub {
        seen: Arc::default(),
        status: 200,
        raw,
        overflow: false,
        pending: false,
    }
}

#[tokio::test]
async fn fixed_profiles_send_only_the_required_chat_fields() {
    for profile in [Profile::OpenaiLuna, Profile::DeepseekFlash] {
        let raw = encoded(&response(profile, r#"{"action":"increment"}"#)).unwrap();
        let transport = stub(raw);
        let seen = transport.seen.clone();
        let mut provider = Choice::new(transport, config(profile)).unwrap();
        assert_eq!(provider.select(request()).await.unwrap(), "increment");
        let bytes = &seen.lock().unwrap()[0];
        let body = decode(bytes).unwrap();
        assert_eq!(body["model"], profile.model());
        assert_eq!(body["response_format"], json!({"type":"json_object"}));
        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
        assert_eq!(
            body["messages"][1]["content"],
            String::from_utf8(encoded(&request()).unwrap()).unwrap()
        );
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(
            system.starts_with("Choose one next action toward the objective in the observation.")
        );
        assert!(system.contains("{\"action\":\"candidate_id\"}"));
        if profile == Profile::OpenaiLuna {
            assert_eq!(body["reasoning_effort"], "none");
            assert_eq!(body["max_completion_tokens"], 64);
            assert_eq!(body["service_tier"], "default");
            assert_eq!(body["store"], false);
            assert!(body.get("thinking").is_none() && body.get("max_tokens").is_none());
            assert_eq!(body.as_object().unwrap().len(), 7);
        } else {
            assert_eq!(body["thinking"], json!({"type":"disabled"}));
            assert_eq!(body["max_tokens"], 64);
            assert!(
                body.get("reasoning_effort").is_none()
                    && body.get("max_completion_tokens").is_none()
                    && body.get("service_tier").is_none()
                    && body.get("store").is_none()
            );
            assert_eq!(body.as_object().unwrap().len(), 5);
        }
        let receipt = provider.take_evidence().pop().unwrap();
        assert_eq!(receipt["request_sha256"], sha256(bytes));
        assert_eq!(receipt["requested_model"], profile.model());
        assert_eq!(receipt["reported_model"], profile.model());
        assert_eq!(receipt["usage"]["cache_read_tokens"], 20);
        if profile == Profile::DeepseekFlash {
            assert_eq!(receipt["usage"]["cache_miss_tokens"], 80);
        }
        assert!(receipt["usage"]["cache_write_tokens"].is_null());
        assert_eq!(receipt["usage"]["reasoning_tokens"], 2);
        assert!(receipt["estimated_usd"].is_null() && receipt["billed_usd"].is_null());
        assert_eq!(receipt["outcome"], "accepted");
        assert!(receipt["reserved_usd"].as_f64().unwrap() > 0.0);
        assert!(encoded(&receipt).unwrap().len() < provider.evidence_limit());
    }
}

#[tokio::test]
async fn unoffered_malformed_and_duplicate_actions_fail_with_raw_usage_retained() {
    let profile = Profile::DeepseekFlash;
    for (content, code) in [
        (r#"{"action":"other"}"#, "unknown_candidate"),
        (r#"{"action":42}"#, "invalid_provider_choice"),
        (
            r#"{"action":"increment","extra":true}"#,
            "invalid_provider_choice",
        ),
        (r#"{"action":"increment","action":"stop"}"#, "invalid_json"),
        (r#"not json"#, "invalid_json"),
    ] {
        let raw = encoded(&response(profile, content)).unwrap();
        let mut provider = Choice::new(stub(raw.clone()), config(profile)).unwrap();
        assert_eq!(provider.select(request()).await.unwrap_err().0, code);
        let receipt = provider.take_evidence().pop().unwrap();
        assert_eq!(receipt["outcome"], "failed");
        assert_eq!(receipt["error"], code);
        assert_eq!(receipt["usage_status"], "valid");
        assert_eq!(receipt["usage"]["input_tokens"], 100);
        assert_eq!(receipt["response_raw"], String::from_utf8(raw).unwrap());
    }
    let raw = br#"{"model":"deepseek-flash","model":"deepseek-flash"}"#.to_vec();
    let mut provider = Choice::new(stub(raw.clone()), config(profile)).unwrap();
    assert_eq!(
        provider.select(request()).await.unwrap_err().0,
        "invalid_json"
    );
    assert_eq!(
        provider.take_evidence()[0]["response_raw"],
        String::from_utf8(raw).unwrap()
    );
}

#[tokio::test]
async fn model_truncation_refusal_and_usage_fail_closed() {
    let profile = Profile::OpenaiLuna;
    for (kind, code) in [
        ("model", "model_mismatch"),
        ("length", "provider_output_truncated"),
        ("refusal", "provider_refusal"),
        ("usage", "invalid_usage"),
    ] {
        let mut reply = response(profile, r#"{"action":"increment"}"#);
        match kind {
            "model" => reply["model"] = "gpt-6-luna-alias".into(),
            "length" => reply["choices"][0]["finish_reason"] = "length".into(),
            "refusal" => reply["choices"][0]["message"]["refusal"] = "no".into(),
            _ => reply["usage"]["completion_tokens"] = "ten".into(),
        }
        let raw = encoded(&reply).unwrap();
        let mut provider = Choice::new(stub(raw.clone()), config(profile)).unwrap();
        assert_eq!(provider.select(request()).await.unwrap_err().0, code);
        let receipt = provider.take_evidence().pop().unwrap();
        assert_eq!(receipt["outcome"], "failed");
        assert_eq!(receipt["response_raw"], String::from_utf8(raw).unwrap());
        if kind == "usage" {
            assert!(receipt["usage"].is_null());
        }
    }
    let mut missing = response(profile, r#"{"action":"stop"}"#);
    missing.as_object_mut().unwrap().remove("usage");
    let mut provider = Choice::new(stub(encoded(&missing).unwrap()), config(profile)).unwrap();
    assert_eq!(provider.select(request()).await.unwrap(), "stop");
    let receipt = provider.take_evidence().pop().unwrap();
    assert_eq!(receipt["usage_status"], "missing");
    assert!(receipt["usage"].is_null() && receipt["estimated_usd"].is_null());
    for (profile, field, value) in [
        (Profile::OpenaiLuna, "cache_write_tokens", json!(90)),
        (
            Profile::DeepseekFlash,
            "prompt_cache_miss_tokens",
            json!(79),
        ),
        (
            Profile::DeepseekFlash,
            "prompt_cache_miss_tokens",
            json!(u32::MAX),
        ),
    ] {
        let mut reply = response(profile, r#"{"action":"increment"}"#);
        if profile == Profile::OpenaiLuna {
            reply["usage"]["prompt_tokens_details"][field] = value;
        } else {
            reply["usage"][field] = value;
        }
        let mut provider = Choice::new(stub(encoded(&reply).unwrap()), config(profile)).unwrap();
        assert_eq!(
            provider.select(request()).await.unwrap_err().0,
            "invalid_usage"
        );
        let receipt = provider.take_evidence().pop().unwrap();
        assert_eq!(receipt["usage_status"], "invalid");
        assert!(receipt["usage"].is_null());
    }
}

#[tokio::test]
async fn byte_limits_and_http_failures_retain_bounded_raw_reply() {
    let profile = Profile::DeepseekFlash;
    let transport = stub(vec![]);
    let seen = transport.seen.clone();
    let mut provider = Choice::new(transport, config(profile)).unwrap();
    let mut state = request();
    state["observation"]["large"] = "x".repeat(4096).into();
    assert_eq!(
        provider.select(state).await.unwrap_err().0,
        "invalid_provider_request"
    );
    assert_eq!(provider.calls(), 0);
    assert!(seen.lock().unwrap().is_empty());
    let mut transport = stub(vec![b'x'; MAX_BYTES + 1]);
    transport.overflow = true;
    let mut provider = Choice::new(transport, config(profile)).unwrap();
    assert_eq!(
        provider.select(request()).await.unwrap_err().0,
        "provider_response_size"
    );
    let receipt = provider.take_evidence().pop().unwrap();
    assert_eq!(receipt["response_raw"].as_str().unwrap().len(), MAX_BYTES);
    assert_eq!(receipt["response_truncated"], true);
    let mut transport = stub(br#"{"error":"rate limited"}"#.to_vec());
    transport.status = 429;
    let mut provider = Choice::new(transport, config(profile)).unwrap();
    assert_eq!(
        provider.select(request()).await.unwrap_err().0,
        "provider_http_status"
    );
    let receipt = provider.take_evidence().pop().unwrap();
    assert_eq!(receipt["http_status"], 429);
    assert!(
        receipt["response_raw"]
            .as_str()
            .unwrap()
            .contains("rate limited")
    );
}

#[tokio::test]
async fn interrupted_call_and_first_mismatch_keep_exact_evidence() {
    let profile = Profile::DeepseekFlash;
    let mut transport = stub(vec![]);
    transport.pending = true;
    let seen = transport.seen.clone();
    let mut matched = MatchedProvider::new(
        Box::new(Choice::new(transport, config(profile)).unwrap()),
        None,
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(20), matched.select(request()))
            .await
            .is_err()
    );
    let receipts = matched.take_evidence();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0]["outcome"], "interrupted");
    assert!(receipts[0]["usage"].is_null());
    assert_eq!(matched.elapsed_ms.len(), 1);
    assert_eq!(seen.lock().unwrap().len(), 1);
    let transport = stub(vec![]);
    let seen = transport.seen.clone();
    let mut matched = MatchedProvider::new(
        Box::new(Choice::new(transport, config(profile)).unwrap()),
        Some("0".repeat(64)),
    );
    assert_eq!(
        matched.select(request()).await.unwrap_err().0,
        "comparison_initial_mismatch"
    );
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(matched.first, Some(sha256(&encoded(&request()).unwrap())));
    assert!(matched.elapsed_ms.is_empty() && matched.take_evidence().is_empty());
}

#[test]
fn config_is_strict_and_versioned() {
    for bad in [
        json!({"version":2,"profile":"openai_luna"}),
        json!({"version":1,"profile":"openai_luna","request_limit":0}),
        json!({"version":1,"profile":"deepseek_flash","request_limit":13}),
        json!({"version":1,"profile":"deepseek_flash","request_bytes":16385}),
    ] {
        let config: Config = serde_json::from_value(bad).unwrap();
        assert!(config.validate().is_err());
    }
    for bad in [
        json!({"profile":"openai_luna"}),
        json!({"version":1,"profile":"old_flash"}),
        json!({"version":1,"profile":"openai_luna","tools":[]}),
    ] {
        assert!(serde_json::from_value::<Config>(bad).is_err());
    }
    let config: Config =
        serde_json::from_value(json!({"version":1,"profile":"openai_luna"})).unwrap();
    assert_eq!(config.request_bytes, MAX_BYTES);
    assert_eq!(config.request_limit, 6);
}

#[test]
fn cli_matches_first_request_excludes_keys_and_replays_through_real_pipes() {
    let temp = tempfile::tempdir().unwrap();
    let worker = temp.path().join("worker.py");
    let fixture = format!("{}/tests/rust_fixture.py", env!("CARGO_MANIFEST_DIR"));
    let source = format!(
        "import os, runpy, sys\nassert not any(k in os.environ for k in ('TYPESAFE_API_KEY', 'OPENAI_API_KEY', 'DEEPSEEK_API_KEY'))\nsys.argv = [{}, 'normal']\nsys.path.insert(0, os.path.dirname(sys.argv[0]))\nrunpy.run_path(sys.argv[0], run_name='__main__')\n",
        serde_json::to_string(&fixture).unwrap()
    );
    std::fs::write(&worker, source).unwrap();
    let binary = env!("CARGO_BIN_EXE_redshirt");
    let invoke = |name: &str, mode: &[&str]| {
        let output = temp.path().join(name);
        let mut command = std::process::Command::new(binary);
        command
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("TYPESAFE_API_KEY", "synthetic-jev-key")
            .env("OPENAI_API_KEY", "synthetic-openai-key")
            .env("DEEPSEEK_API_KEY", "synthetic-deepseek-key")
            .arg("--output")
            .arg(&output)
            .args(mode)
            .arg("--adapter")
            .arg("python3")
            .arg(&worker);
        let result = command.output().unwrap();
        (output, result)
    };
    let (baseline, result) = invoke("baseline", &["--remote-provider"]);
    assert!(
        result.status.success(),
        "stdout={} stderr={} report={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
        std::fs::read_to_string(baseline.join("report.json")).unwrap_or_default()
    );
    let rows: Vec<Value> = std::fs::read_to_string(baseline.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|line| decode(line.as_bytes()).unwrap())
        .collect();
    let first = rows.iter().find(|row| row["event"] == "request").unwrap()["data"].clone();
    let digest = sha256(&encoded(&first).unwrap());
    let (matched, result) = invoke(
        "matched",
        &["--expected-initial", &digest, "--remote-provider"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let selection = decode(&std::fs::read(matched.join("selection.json")).unwrap()).unwrap();
    assert_eq!(selection["expected_initial_sha256"], digest);
    assert_eq!(selection["first_request_sha256"], digest);
    assert_eq!(selection["elapsed_ms"].as_array().unwrap().len(), 2);
    let report = decode(&std::fs::read(matched.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["stop"], "selector_stop");
    let (mismatch, result) = invoke(
        "mismatch",
        &["--expected-initial", &"0".repeat(64), "--remote-provider"],
    );
    assert!(!result.status.success());
    let report = decode(&std::fs::read(mismatch.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["stop"], "comparison_initial_mismatch");
    assert_eq!(report["requests"], 1);
    assert_eq!(report["attempted_inputs"], 0);
    let selection = decode(&std::fs::read(mismatch.join("selection.json")).unwrap()).unwrap();
    assert!(selection["elapsed_ms"].as_array().unwrap().is_empty());
    assert_eq!(selection["first_request_sha256"], digest);
    let replay_path = matched.join("replay.json");
    let (replayed, result) = invoke("replayed", &["--replay", replay_path.to_str().unwrap()]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report = decode(&std::fs::read(replayed.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["replay_complete"], true);
    assert_eq!(report["requests"], 0);
    assert!(!replayed.join("selection.json").exists());
    let occupied = temp.path().join("occupied");
    std::fs::create_dir(&occupied).unwrap();
    std::fs::write(occupied.join("sentinel"), b"do not change").unwrap();
    let (_, result) = invoke("occupied", &["--remote-provider"]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        std::fs::read(occupied.join("sentinel")).unwrap(),
        b"do not change"
    );
    assert_eq!(std::fs::read_dir(&occupied).unwrap().count(), 1);
}

#[test]
fn cli_refuses_choice_without_transport_or_key_before_adapter_launch() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("choice.json");
    std::fs::write(
        &config,
        br#"{"version":1,"profile":"openai_luna","request_limit":2}"#,
    )
    .unwrap();
    let output = temp.path().join("run");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_redshirt"))
        .env_remove("OPENAI_API_KEY")
        .arg("--output")
        .arg(&output)
        .arg("--choice")
        .arg(&config)
        .arg("--adapter")
        .arg("must-not-launch-this-command")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    let expected = if cfg!(feature = "choice-http") {
        "missing_or_invalid_key"
    } else {
        "choice_http_feature_required"
    };
    assert!(String::from_utf8_lossy(&result.stderr).contains(expected));
    assert!(!output.exists());
}
