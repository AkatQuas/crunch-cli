//! Developer benchmark. Not shipped in release archives.
//!
//! From a clone with submodules:
//!
//!   cargo run --release --bin crunch-bench
//!   cargo run --release --bin crunch-bench -- path/to/pngs
//!   cargo run --release --bin crunch-bench -- --compare /path/to/old-crunch

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crunch_cli::{optimize, DEFAULT_BENCH_DIR};
use pngdeer::Settings;

fn main() {
    if let Err(err) = run() {
        eprintln!("crunch-bench: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut dir = PathBuf::from(DEFAULT_BENCH_DIR);
    let mut compare = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--compare" => {
                compare = Some(PathBuf::from(args.next().ok_or("--compare needs a path")?));
            }
            "-h" | "--help" => {
                println!(
                    "usage: crunch-bench [DIR] [--compare PATH]\n\
                     DIR defaults to {DEFAULT_BENCH_DIR}"
                );
                return Ok(());
            }
            _ if arg.starts_with('-') => return Err(format!("unknown argument: {arg}")),
            _ => dir = PathBuf::from(arg),
        }
    }
    if let Some(path) = &compare {
        if !path.is_file() {
            return Err(format!(
                "comparison command not found: {} (pass the old pngquant + zopfli crunch binary)",
                path.display()
            ));
        }
    }
    if !dir.is_dir() {
        return Err(format!(
            "{} is not a directory. Clone with --recurse-submodules, or pass a PNG directory.",
            dir.display()
        ));
    }
    let mut files = Vec::new();
    collect(&dir, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(format!("no PNG files in {}", dir.display()));
    }

    let compare_header = if compare.is_some() {
        " compare cmp_s"
    } else {
        ""
    };
    println!(
        "{:<40} {:>8} {:>8} {:>7} {:>7} {:>8} {:>8} {:>8} {:>8}{compare_header}",
        "file", "in", "pngdeer", "score", "target", "deer_s", "zopfli", "zop_s", "final"
    );
    let mut totals = Totals::default();
    for file in &files {
        let row = bench_file(file, compare.as_deref())?;
        totals.add(&row);
        print!(
            "{:<40} {:>8} {:>8} {:>7.2} {:>7.2} {:>8.2} {:>8} {:>8} {:>8}",
            row.name,
            row.input,
            row.pngdeer,
            row.score,
            row.target,
            row.pngdeer_secs,
            row.zopfli,
            row.zopfli_secs,
            row.final_len
        );
        if let (Some(len), Some(secs)) = (row.compare_len, row.compare_secs) {
            print!(" {len:>8} {secs:>8.2}");
        }
        println!();
    }
    println!(
        "{:<40} {:>8} {:>8} {:>7} {:>7} {:>8.2} {:>8} {:>8.2} {:>8}",
        "total",
        totals.input,
        totals.pngdeer,
        "",
        "",
        totals.pngdeer_secs,
        totals.zopfli,
        totals.zopfli_secs,
        totals.final_len
    );
    Ok(())
}

struct Row {
    name: String,
    input: usize,
    pngdeer: usize,
    score: f64,
    target: f64,
    pngdeer_secs: f64,
    zopfli: String,
    zopfli_secs: String,
    final_len: usize,
    compare_len: Option<usize>,
    compare_secs: Option<f64>,
}

#[derive(Default)]
struct Totals {
    input: usize,
    pngdeer: usize,
    pngdeer_secs: f64,
    zopfli: usize,
    zopfli_secs: f64,
    final_len: usize,
}

impl Totals {
    fn add(&mut self, row: &Row) {
        self.input += row.input;
        self.pngdeer += row.pngdeer;
        self.pngdeer_secs += row.pngdeer_secs;
        self.final_len += row.final_len;
        if let Ok(bytes) = row.zopfli.parse::<usize>() {
            self.zopfli += bytes;
        }
        if let Ok(secs) = row.zopfli_secs.parse::<f64>() {
            self.zopfli_secs += secs;
        }
    }
}

fn bench_file(file: &Path, compare: Option<&Path>) -> Result<Row, String> {
    let input = std::fs::read(file).map_err(|err| format!("{}: {err}", file.display()))?;
    let optimized = optimize(&input, &Settings::default())?;
    let (zopfli, zopfli_secs) = if optimized.animated {
        ("skipped".to_string(), "-".to_string())
    } else {
        (
            optimized.output.len().to_string(),
            format!("{:.2}", optimized.zopfli_secs.unwrap_or(0.0)),
        )
    };
    let (compare_len, compare_secs) = if let Some(bin) = compare {
        let (len, secs) = run_compare(bin, file)?;
        (Some(len), Some(secs))
    } else {
        (None, None)
    };
    Ok(Row {
        name: file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_string(),
        input: optimized.input_len,
        pngdeer: optimized.pngdeer_len,
        score: optimized.score,
        target: optimized.target,
        pngdeer_secs: optimized.pngdeer_secs,
        zopfli,
        zopfli_secs,
        final_len: if optimized.output.len() < optimized.input_len {
            optimized.output.len()
        } else {
            optimized.input_len
        },
        compare_len,
        compare_secs,
    })
}

fn run_compare(bin: &Path, file: &Path) -> Result<(usize, f64), String> {
    let dir = std::env::temp_dir().join(format!("crunch-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let name = file.file_name().ok_or("missing file name")?;
    let dest = dir.join(name);
    std::fs::copy(file, &dest).map_err(|err| format!("{}: {err}", file.display()))?;
    let started = Instant::now();
    let status = Command::new(bin)
        .arg(&dest)
        .status()
        .map_err(|err| format!("failed to run {}: {err}", bin.display()))?;
    let secs = started.elapsed().as_secs_f64();
    if !status.success() {
        return Err(format!(
            "{} failed on {} ({status})",
            bin.display(),
            file.display()
        ));
    }
    let len = std::fs::metadata(&dest)
        .map_err(|err| err.to_string())?
        .len() as usize;
    let _ = std::fs::remove_file(&dest);
    Ok((len, secs))
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))? {
        let path = entry.map_err(|err| err.to_string())?.path();
        if path.is_dir() {
            collect(&path, files)?;
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            files.push(path);
        }
    }
    Ok(())
}
