use redshirt::{
    CancellationToken, Result, Stop,
    comparison::{self, Mode},
};
use std::path::PathBuf;

async fn execute() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut manifest, mut output, mut mode) = (None, None, Mode::Mock);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => manifest = args.next().map(PathBuf::from),
            "--output" => output = args.next().map(PathBuf::from),
            "--live" if mode == Mode::Mock => mode = Mode::Live,
            _ => return Err("invalid_arguments".into()),
        }
    }
    let manifest =
        comparison::read_manifest(&manifest.ok_or_else(|| Stop::from("manifest_required"))?)?;
    let output = output.ok_or_else(|| Stop::from("output_required"))?;
    let cancel = CancellationToken::new();
    let signal_cancel = cancel.clone();
    let signal = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_cancel.cancel();
        }
    });
    let result = comparison::campaign(manifest, &output, mode, &cancel).await;
    signal.abort();
    let summary = result?;
    println!(
        "{}",
        serde_json::json!({"complete":summary["complete"],"mode":summary["mode"],"runs":summary["runs"].as_array().map(Vec::len)})
    );
    Ok(())
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = execute().await {
        eprintln!("redshirt-compare: {error}");
        std::process::exit(2);
    }
}
