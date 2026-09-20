use redshirt::{evidence::load, process::ProcessAdapter, *};
use std::path::PathBuf;

async fn main_result() -> Result<bool> {
    let mut args = std::env::args().skip(1);
    let (mut output, mut script, mut replay, mut limits_path) = (None, None, None, None);
    let mut remote = false;
    let mut argv = vec![];
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().map(PathBuf::from),
            "--script" => script = args.next().map(PathBuf::from),
            "--replay" => replay = args.next().map(PathBuf::from),
            "--limits" => limits_path = args.next().map(PathBuf::from),
            "--remote-provider" => remote = true,
            "--adapter" => {
                argv = args.collect();
                break;
            }
            _ => return Err("invalid_arguments".into()),
        }
    }
    require(
        usize::from(script.is_some()) + usize::from(replay.is_some()) + usize::from(remote) == 1,
        "choose_one_mode",
    )?;
    let output = output.ok_or_else(|| Stop::from("output_required"))?;
    let limits: Limits = if let Some(path) = limits_path {
        serde_json::from_value(load(&path, 4096)?).map_err(|_| Stop::from("invalid_limits"))?
    } else {
        Limits::default()
    };
    limits.validate()?;
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
    let cancel = CancellationToken::new();
    let signal_cancel = cancel.clone();
    let signal = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_cancel.cancel();
        }
    });
    let (mut adapter, mut provider) = ProcessAdapter::spawn(&argv)?;
    let selector: Option<&mut dyn Provider> = if remote {
        Some(&mut provider)
    } else {
        script.as_mut().map(|p| p as &mut dyn Provider)
    };
    let result = run(&mut adapter, selector, saved, &output, limits, &cancel).await;
    adapter.terminate().await;
    signal.abort();
    let report = result?;
    println!(
        "{}",
        serde_json::json!({"stop":report["stop"],"verified":report["final"]["ok"],
        "cleanup":report["cleanup"],"attempted_inputs":report["attempted_inputs"],"requests":report["requests"]})
    );
    Ok(
        (report["stop"] == "selector_stop" || report["stop"] == "replay_complete")
            && report["final"]["ok"] == true
            && report["cleanup"] == true,
    )
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
