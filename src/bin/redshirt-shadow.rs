use redshirt::{
    CancellationToken, Result, Stop,
    context::{self, shadow},
    jev,
};
use std::path::PathBuf;

async fn execute() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut manifest, mut repo, mut output, mut replay) = (None, None, None, None);
    let (mut preflight, mut live) = (false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" if manifest.is_none() => manifest = args.next().map(PathBuf::from),
            "--repo" if repo.is_none() => repo = args.next().map(PathBuf::from),
            "--output" if output.is_none() => output = args.next().map(PathBuf::from),
            "--replay" if replay.is_none() => replay = args.next().map(PathBuf::from),
            "--preflight" if !preflight => preflight = true,
            "--live" if !live => live = true,
            _ => return Err("shadow_invalid_arguments".into()),
        }
    }
    let campaign_mode = replay.is_none() && !preflight;
    let result = if let Some(saved) = replay {
        if manifest.is_some() || repo.is_some() || output.is_some() || preflight || live {
            return Err("shadow_invalid_arguments".into());
        }
        shadow::replay(&saved)?
    } else {
        let manifest = shadow::read_manifest(
            &manifest.ok_or_else(|| Stop::from("shadow_manifest_required"))?,
        )?;
        let repo = repo.ok_or_else(|| Stop::from("shadow_repo_required"))?;
        if preflight {
            if output.is_some() || live {
                return Err("shadow_invalid_arguments".into());
            }
            shadow::preflight(&manifest, &repo)?
        } else {
            let output = output.ok_or_else(|| Stop::from("shadow_output_required"))?;
            let cancel = CancellationToken::new();
            let signal_cancel = cancel.clone();
            let signal = tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_cancel.cancel();
                }
            });
            let result = if live {
                #[cfg(feature = "jev-http")]
                {
                    let key = std::env::var("TYPESAFE_API_KEY")
                        .map_err(|_| Stop::from("missing_api_key"))?;
                    let provider = jev::Jev::live(key, manifest.provider_config())?;
                    shadow::campaign(manifest, &repo, provider, &output, true, &cancel).await
                }
                #[cfg(not(feature = "jev-http"))]
                {
                    Err(Stop::from("jev_http_feature_required"))
                }
            } else {
                let provider = jev::Jev::new(context::MockTransport, manifest.provider_config())?;
                shadow::campaign(manifest, &repo, provider, &output, false, &cancel).await
            };
            signal.abort();
            result?
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|_| Stop::from("shadow_report_json"))?
    );
    if campaign_mode && result["complete"] == false {
        return Err("shadow_campaign_incomplete".into());
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = execute().await {
        eprintln!("redshirt-shadow: {error}");
        std::process::exit(2);
    }
}
