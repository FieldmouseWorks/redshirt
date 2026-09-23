use redshirt::{Result, Stop, context::packet};
use std::path::PathBuf;

fn execute() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let action = args
        .next()
        .ok_or_else(|| Stop::from("packet_action_required"))?;
    let (mut repo, mut spec, mut output, mut packet) = (None, None, None, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" if repo.is_none() => repo = args.next().map(PathBuf::from),
            "--spec" if spec.is_none() => spec = args.next().map(PathBuf::from),
            "--output" if output.is_none() => output = args.next().map(PathBuf::from),
            "--packet" if packet.is_none() => packet = args.next().map(PathBuf::from),
            _ => return Err("packet_invalid_arguments".into()),
        }
    }
    let repo = repo.ok_or_else(|| Stop::from("packet_repo_required"))?;
    let summary = match action.as_str() {
        "build" if packet.is_none() => {
            let spec = packet::read_spec(&spec.ok_or_else(|| Stop::from("packet_spec_required"))?)?;
            let output = output.ok_or_else(|| Stop::from("packet_output_required"))?;
            let built = packet::build(&repo, &spec)?;
            packet::write_packet(&output, &built)?;
            built.build_summary()?
        }
        "verify" if spec.is_none() && output.is_none() => {
            let packet =
                packet::read_packet(&packet.ok_or_else(|| Stop::from("packet_input_required"))?)?;
            packet::verify(&repo, &packet)?
        }
        _ => return Err("packet_invalid_arguments".into()),
    };
    println!(
        "{}",
        serde_json::to_string(&summary).map_err(|_| Stop::from("packet_json"))?
    );
    Ok(())
}

fn main() {
    if let Err(error) = execute() {
        eprintln!("redshirt-packet: {error}");
        std::process::exit(2);
    }
}
