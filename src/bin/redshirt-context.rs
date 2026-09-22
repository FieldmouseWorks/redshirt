use redshirt::{CancellationToken, Result, Stop, context, jev};
use std::path::PathBuf;

async fn execute() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut manifest, mut oracle, mut output, mut replay) = (None, None, None, None);
    let mut codex_executable = None;
    let (mut live, mut preflight) = (false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" if manifest.is_none() => manifest = args.next().map(PathBuf::from),
            "--oracle" if oracle.is_none() => oracle = args.next().map(PathBuf::from),
            "--output" if output.is_none() => output = args.next().map(PathBuf::from),
            "--replay" if replay.is_none() => replay = args.next().map(PathBuf::from),
            "--codex-executable" if codex_executable.is_none() => {
                codex_executable = args.next().map(PathBuf::from)
            }
            "--live" if !live => live = true,
            "--preflight" if !preflight => preflight = true,
            _ => return Err("invalid_arguments".into()),
        }
    }
    let result = if let Some(replay) = replay {
        if live
            || preflight
            || manifest.is_some()
            || oracle.is_some()
            || output.is_some()
            || codex_executable.is_some()
        {
            return Err("invalid_arguments".into());
        }
        context::replay(&replay)?
    } else {
        let (manifest, oracle) = context::read_inputs(
            &manifest.ok_or_else(|| Stop::from("manifest_required"))?,
            &oracle.ok_or_else(|| Stop::from("oracle_required"))?,
        )?;
        if preflight {
            if live || output.is_some() || codex_executable.is_some() {
                return Err("invalid_arguments".into());
            }
            context::preflight(&manifest, &oracle)?
        } else {
            let output = output.ok_or_else(|| Stop::from("output_required"))?;
            let cancel = CancellationToken::new();
            let signal_cancel = cancel.clone();
            let signal = tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_cancel.cancel();
                }
            });
            let config = manifest.provider_config();
            let run = if live {
                #[cfg(feature = "jev-http")]
                {
                    let key = std::env::var("TYPESAFE_API_KEY")
                        .map_err(|_| Stop::from("missing_api_key"))?;
                    let provider = jev::Jev::live(key, config)?;
                    if let Some(profile) = manifest.diagnostic.clone() {
                        let executable = codex_executable
                            .ok_or_else(|| Stop::from("codex_executable_required"))?;
                        let transport = context::codex::Cli::new(executable, profile.clone())?;
                        let diagnostic = context::codex::Diagnostic::new(transport, profile)?;
                        context::campaign_with_diagnostic(
                            manifest, oracle, provider, diagnostic, &output, true, &cancel,
                        )
                        .await
                    } else {
                        if codex_executable.is_some() {
                            return Err("invalid_arguments".into());
                        }
                        context::campaign(manifest, oracle, provider, &output, true, &cancel).await
                    }
                }
                #[cfg(not(feature = "jev-http"))]
                {
                    Err(Stop::from("jev_http_feature_required"))
                }
            } else {
                if codex_executable.is_some() {
                    return Err("invalid_arguments".into());
                }
                let provider = jev::Jev::new(context::MockTransport, config)?;
                if let Some(profile) = manifest.diagnostic.clone() {
                    let diagnostic =
                        context::codex::Diagnostic::new(context::codex::Mock, profile)?;
                    context::campaign_with_diagnostic(
                        manifest, oracle, provider, diagnostic, &output, false, &cancel,
                    )
                    .await
                } else {
                    context::campaign(manifest, oracle, provider, &output, false, &cancel).await
                }
            };
            signal.abort();
            run?
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|_| Stop::from("invalid_json"))?
    );
    if result["complete"] == false && !preflight {
        return Err("context_campaign_incomplete".into());
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = execute().await {
        eprintln!("redshirt-context: {error}");
        std::process::exit(2);
    }
}
