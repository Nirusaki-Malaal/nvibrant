mod backend;
mod gui;
mod install;
mod runtime;
mod settings;
use anyhow::{Context, Result, bail, ensure};
use std::{collections::BTreeMap, env, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("NVibrant: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "NVibrant {}\n\nUsage: nvibrant [--gui | --demo | --restore | --list-json | VALUES...]\n\n  --gui              Open the native GTK interface\n  --demo             Preview without hardware or saved settings\n  --restore          Restore saved colours at desktop login, without a window\n  --list-all-json    Auto-detect connected displays on all NVIDIA GPUs\n  --list-json        Read-only output discovery for one GPU\n  --gpu INDEX        Select NVIDIA GPU (0..15)\n  --install PREFIX   Install this executable and desktop/login entries\n  --uninstall PREFIX Remove this installation, preserving preferences\n  --smoke-test       Exercise GTK controls with simulated hardware\n  --version          Print version\n\nCLI values: -1024 (grayscale) to 1023 (maximum); --skip leaves an output alone.\nNVIDIA_GPU and ATTRIBUTE are supported for CLI compatibility.",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if args == ["--version"] {
        println!(
            "NVibrant {} (Rust)\nNVIDIA headers: {}",
            env!("CARGO_PKG_VERSION"),
            env!("NVIBRANT_BUILD_DRIVER")
        );
        return Ok(());
    }
    for (flag, remove) in [("--install", false), ("--uninstall", true)] {
        if args.first().map(String::as_str) == Some(flag) {
            ensure!(args.len() == 2, "{flag} requires one absolute prefix");
            return if remove {
                install::uninstall(&PathBuf::from(&args[1]))
            } else {
                install::install(&PathBuf::from(&args[1]))
            };
        }
    }
    if args == ["--restore"] {
        return runtime::restore_login();
    }
    if args.is_empty() || args == ["--gui"] || args == ["--demo"] || args == ["--smoke-test"] {
        return gui::run(
            args == ["--demo"] || args == ["--smoke-test"],
            args == ["--smoke-test"],
        );
    }
    if args == ["--list-all-json"] {
        let displays = runtime::discover_connected(&runtime::Native, &backend::gpu_indices()?)?;
        println!("{}", serde_json::to_string(&displays)?);
        return Ok(());
    }
    let mut gpu: u8 = env::var("NVIDIA_GPU")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .context("Invalid NVIDIA_GPU")?;
    if let Some(index) = args.iter().position(|a| a == "--gpu") {
        gpu = args
            .get(index + 1)
            .context("--gpu needs an index")?
            .parse()
            .context("Invalid GPU index")?;
        args.drain(index..=index + 1);
    }
    ensure!(gpu <= 15, "GPU index must be between 0 and 15");
    if args == ["--list-json"] {
        println!(
            "{}",
            serde_json::to_string(&backend::Discovery {
                ports: backend::discover(gpu)?
            })?
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--apply-json") {
        ensure!(args.len() == 2, "--apply-json needs one JSON object");
        let values = serde_json::from_str(&args[1])?;
        settings::validate_values(&values)?;
        backend::apply(gpu, &values, "vibrance")?;
    } else {
        let attribute = env::var("ATTRIBUTE").unwrap_or_else(|_| "vibrance".into());
        let (min, max, fallback) = match attribute.as_str() {
            "vibrance" => (-1024, 1023, 0),
            "dithering" => (0, 2, 2),
            "dithering-mode" | "dithering-depth" => (0, 3, 0),
            _ => bail!("Unknown ATTRIBUTE: {attribute}"),
        };
        // Parse everything before opening hardware; unknown flags never become zero.
        let parsed = args
            .iter()
            .map(|v| {
                if v == "--skip" {
                    Ok(None)
                } else {
                    Ok(Some(
                        v.parse::<i32>()
                            .context("Expected a numeric value or --skip")?
                            .clamp(min, max),
                    ))
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let ports = backend::discover(gpu)?;
        ensure!(
            parsed.len() <= ports.len(),
            "More values than NVIDIA outputs"
        );
        let values: BTreeMap<_, _> = ports
            .iter()
            .filter(|p| p.connected)
            .filter_map(|p| {
                parsed
                    .get(p.index)
                    .copied()
                    .unwrap_or(Some(fallback))
                    .map(|v| (p.key(), v))
            })
            .collect();
        if values.is_empty() {
            return Ok(());
        }
        backend::apply(gpu, &values, &attribute)?;
    }
    println!("Success");
    Ok(())
}
