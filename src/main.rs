use redshirt::{
    comparison::MatchedProvider,
    evidence::{decode, encoded, load},
    process::ProcessAdapter,
    *,
};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

async fn main_result() -> Result<bool> {
    let mut args = std::env::args().skip(1);
    let (mut output, mut script, mut replay, mut limits_path) = (None, None, None, None);
    let mut remote = false;
    let mut stdio = false;
    let mut limits_json = None;
    let mut jev_config = None;
    let mut choice_config = None;
    let mut expected_initial = None;
    let mut argv = vec![];
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().map(PathBuf::from),
            "--script" => script = args.next().map(PathBuf::from),
            "--replay" => replay = args.next().map(PathBuf::from),
            "--limits" => limits_path = args.next().map(PathBuf::from),
            "--limits-json" => {
                limits_json = Some(args.next().ok_or_else(|| Stop::from("invalid_arguments"))?)
            }
            "--remote-provider" => remote = true,
            "--stdio" => stdio = true,
            "--jev" => {
                jev_config = Some(PathBuf::from(
                    args.next().ok_or_else(|| Stop::from("invalid_arguments"))?,
                ))
            }
            "--choice" => {
                choice_config = Some(PathBuf::from(
                    args.next().ok_or_else(|| Stop::from("invalid_arguments"))?,
                ))
            }
            "--expected-initial" => {
                expected_initial = Some(args.next().ok_or_else(|| Stop::from("invalid_arguments"))?)
            }
            "--adapter" => {
                argv = args.collect();
                break;
            }
            _ => return Err("invalid_arguments".into()),
        }
    }
    require(
        usize::from(script.is_some())
            + usize::from(replay.is_some())
            + usize::from(remote)
            + usize::from(stdio)
            + usize::from(jev_config.is_some())
            + usize::from(choice_config.is_some())
            == 1,
        "choose_one_mode",
    )?;
    let output = output.ok_or_else(|| Stop::from("output_required"))?;
    require(
        expected_initial.as_ref().is_none_or(|digest: &String| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }),
        "invalid_expected_initial",
    )?;
    require(
        expected_initial.is_none() || remote || jev_config.is_some() || choice_config.is_some(),
        "expected_initial_requires_provider",
    )?;
    require(
        !(limits_path.is_some() && limits_json.is_some()),
        "choose_one_limits_source",
    )?;
    let limits: Limits = if let Some(path) = limits_path {
        serde_json::from_value(load(&path, 4096)?).map_err(|_| Stop::from("invalid_limits"))?
    } else if let Some(raw) = limits_json {
        require(raw.len() <= 4096, "input_size")?;
        serde_json::from_value(decode(raw.as_bytes())?).map_err(|_| Stop::from("invalid_limits"))?
    } else {
        Limits::default()
    };
    limits.validate()?;
    let mut jev_provider: Option<Box<dyn Provider>> = if let Some(path) = jev_config {
        Some(configure_jev(&path)?)
    } else {
        None
    };
    let mut choice_provider: Option<Box<dyn Provider>> = if let Some(path) = choice_config {
        Some(configure_choice(&path, &limits)?)
    } else {
        None
    };
    let saved = replay.map(|p| load(&p, 65536)).transpose()?;
    let mut script = if let Some(path) = script {
        let ids: Vec<String> =
            serde_json::from_value(load(&path, 4096)?).map_err(|_| Stop::from("invalid_script"))?;
        require(
            ids.len() <= 24 && ids.iter().all(|s| s.len() <= 80),
            "invalid_script",
        )?;
        Some(Scripted(ids.into()))
    } else {
        None
    };
    let mut interactive = if stdio {
        Some(interaction::stdio()?)
    } else {
        None
    };
    let cancel = CancellationToken::new();
    let signal_cancel = cancel.clone();
    let signal = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_cancel.cancel();
        }
    });
    let (mut adapter, provider) = ProcessAdapter::spawn_without_env(
        &argv,
        &["TYPESAFE_API_KEY", "OPENAI_API_KEY", "DEEPSEEK_API_KEY"],
    )?;
    let mut matched = if remote {
        Some(MatchedProvider::new(
            Box::new(provider),
            expected_initial.clone(),
        ))
    } else {
        jev_provider
            .take()
            .or_else(|| choice_provider.take())
            .map(|provider| MatchedProvider::new(provider, expected_initial.clone()))
    };
    let selector: Option<&mut dyn Provider> = if let Some(provider) = matched.as_mut() {
        Some(provider)
    } else if let Some(provider) = interactive.as_mut() {
        Some(provider)
    } else {
        script.as_mut().map(|p| p as &mut dyn Provider)
    };
    let result = run(&mut adapter, selector, saved, &output, limits, &cancel).await;
    adapter.terminate().await;
    signal.abort();
    let report = result?;
    if let Some(provider) = matched.as_ref() {
        write_selection(&output, provider)?;
    }
    if let Some(provider) = interactive.as_mut() {
        // Evidence and adapter cleanup are already complete. A vanished or
        // non-reading client must not keep this process alive indefinitely.
        let _ =
            tokio::time::timeout(std::time::Duration::from_secs(5), provider.finish(&report)).await;
    } else {
        println!(
            "{}",
            serde_json::json!({"stop":report["stop"],"verified":report["final"]["ok"],
        "cleanup":report["cleanup"],"attempted_inputs":report["attempted_inputs"],"requests":report["requests"]})
        );
    }
    Ok(
        (report["stop"] == "selector_stop" || report["stop"] == "replay_complete")
            && report["final"]["ok"] == true
            && report["cleanup"] == true,
    )
}

fn write_selection(output: &std::path::Path, provider: &MatchedProvider) -> Result<()> {
    let data = encoded(&serde_json::json!({"version":1,"provider":provider.name(),
        "expected_initial_sha256":provider.expected_first,
        "first_request_sha256":provider.first,"elapsed_ms":provider.elapsed_ms}))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("selection.json"))
        .map_err(|_| Stop::from("evidence_io"))?;
    file.write_all(&data)
        .and_then(|_| file.sync_all())
        .map_err(|_| Stop::from("evidence_io"))
}

#[cfg(feature = "choice-http")]
fn configure_choice(path: &std::path::Path, limits: &Limits) -> Result<Box<dyn Provider>> {
    let config: choice::Config = serde_json::from_value(load(path, 4096)?)
        .map_err(|_| Stop::from("invalid_provider_config"))?;
    config.validate()?;
    require(
        config.request_limit <= limits.requests,
        "provider_request_budget",
    )?;
    let key_name = match config.profile {
        choice::Profile::OpenaiLuna => "OPENAI_API_KEY",
        choice::Profile::DeepseekFlash => "DEEPSEEK_API_KEY",
    };
    let key = std::env::var(key_name).map_err(|_| Stop::from("missing_or_invalid_key"))?;
    Ok(Box::new(choice::Choice::live(key, config)?))
}

#[cfg(not(feature = "choice-http"))]
fn configure_choice(_: &std::path::Path, _: &Limits) -> Result<Box<dyn Provider>> {
    Err("choice_http_feature_required".into())
}

#[cfg(feature = "jev-http")]
fn configure_jev(path: &std::path::Path) -> Result<Box<dyn Provider>> {
    let config: jev::Config = serde_json::from_value(load(path, 8192)?)
        .map_err(|_| Stop::from("invalid_provider_config"))?;
    config.validate()?;
    let key =
        std::env::var("TYPESAFE_API_KEY").map_err(|_| Stop::from("missing_or_invalid_key"))?;
    Ok(Box::new(jev::Jev::live(key, config)?))
}

#[cfg(not(feature = "jev-http"))]
fn configure_jev(_: &std::path::Path) -> Result<Box<dyn Provider>> {
    Err("jev_http_feature_required".into())
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let result = main_result().await;
    match result {
        Ok(true) => (),
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("redshirt: {error}");
            std::process::exit(2);
        }
    }
}
