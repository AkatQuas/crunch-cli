//! `crunch`: replace PNG files in place, unless `-o` names a new file for a single input.

use std::path::PathBuf;
use std::process::ExitCode;

use crunch_cli::{commit, optimize_with, resolve_jobs, Job, PngdeerPalette, ZopfliPng, VERSION};
use pngdeer::Settings;
use rayon::prelude::*;

const HELP: &str = "\
crunch {version}

Lossy PNG compression. Each still image is palette-quantized, then recompressed
with Zopfli. Animated PNGs stay animated and skip Zopfli.

Put the binary on PATH and run it. A result is written only when it is smaller
than the input.

Usage:
  crunch [options] FILE.png...
  crunch [options] DIRECTORY

Options:
  -o, --output PATH   Write one input file to PATH instead of replacing it.
                      Ignored when more than one file is selected, including
                      when the argument is a directory.
  --target N          SSIMULACRA2 score to keep (default: chosen per image)
  -h, --help          Show this help
  -v, --version       Show the version

Several images run at once, using at most one thread pool the size of the CPU.
";

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("crunch: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
    let parsed = parse(args)?;
    let jobs = resolve_jobs(&parsed.paths, parsed.output.as_deref())?;
    let settings = Settings {
        target: parsed.target,
        ..Settings::default()
    };
    let results: Vec<Result<String, String>> = jobs
        .par_iter()
        .map(|job| compress_one(job, &settings))
        .collect();
    let mut failed = false;
    for result in results {
        match result {
            Ok(line) => println!("{line}"),
            Err(err) => {
                failed = true;
                eprintln!("crunch: {err}");
            }
        }
    }
    if failed {
        Err("one or more files failed".into())
    } else {
        Ok(())
    }
}

fn compress_one(job: &Job, settings: &Settings) -> Result<String, String> {
    let input =
        std::fs::read(&job.input).map_err(|err| format!("{}: {err}", job.input.display()))?;
    if !input.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(format!("{} is not a PNG file", job.input.display()));
    }
    let optimized = optimize_with(&input, settings, &PngdeerPalette, &ZopfliPng)?;
    let wrote = commit(&job.input, job.output.as_deref(), &optimized.output)?;
    let (path, len) = if wrote {
        let path = job.output.as_ref().unwrap_or(&job.input);
        (path.display().to_string(), optimized.output.len())
    } else {
        (job.input.display().to_string(), input.len())
    };
    let percent = if input.is_empty() {
        100.0
    } else {
        len as f64 * 100.0 / input.len() as f64
    };
    Ok(format!("{percent:.2}% {path} ({len} bytes)"))
}

struct Parsed {
    output: Option<PathBuf>,
    target: Option<f64>,
    paths: Vec<PathBuf>,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Parsed, String> {
    let args: Vec<String> = args.collect();
    if args.is_empty() {
        print!("{}", HELP.replace("{version}", VERSION));
        std::process::exit(0);
    }
    let mut output = None;
    let mut target = None;
    let mut paths = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{}", HELP.replace("{version}", VERSION));
                std::process::exit(0);
            }
            "-v" | "--version" => {
                println!("crunch {VERSION}");
                std::process::exit(0);
            }
            "-o" | "--output" => {
                index += 1;
                let value = args.get(index).ok_or("--output needs a path")?;
                if output.is_some() {
                    return Err("--output was given more than once".into());
                }
                output = Some(PathBuf::from(value));
            }
            "--target" => {
                index += 1;
                let value = args.get(index).ok_or("--target needs a number")?;
                let score: f64 = value
                    .parse()
                    .map_err(|_| format!("bad --target: {value}"))?;
                target = Some(score);
            }
            _ if arg.starts_with('-') => return Err(format!("unknown argument: {arg}")),
            _ => paths.push(PathBuf::from(arg)),
        }
        index += 1;
    }
    Ok(Parsed {
        output,
        target,
        paths,
    })
}
