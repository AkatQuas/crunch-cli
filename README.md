# crunch

Lossy PNG compression. Each still image is reduced to a palette by [pngdeer](https://github.com/heydeer/pngdeer), then the result is recompressed with [Zopfli](https://github.com/google/zopfli). A file is replaced only when the result is smaller. Animated PNGs stay animated and are not passed through Zopfli.

## Install

Download the archive for your machine from the GitHub Release and put `crunch` on your `PATH`. That is the whole install. You do not need Rust.

| Machine | Archive target |
| --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple silicon | `aarch64-apple-darwin` |
| Linux x64 | `x86_64-unknown-linux-gnu` |
| Linux arm64 | `aarch64-unknown-linux-gnu` |

Windows archives are `.zip` and contain `crunch.exe`. The others are `.tar.gz` and contain `crunch`.

## Usage

```bash
crunch photo.png              # replace photo.png when the result is smaller
crunch a.png b.png            # several files, in parallel
crunch images/                # every PNG under the directory, replaced in place
crunch -o small.png photo.png # write a new file; photo.png is left unchanged
crunch --target 85 photo.png  # keep at least this SSIMULACRA2 score
```

`-o` / `--output` applies only when you pass exactly one file. With several files, or with a directory, `crunch` replaces each PNG in place and ignores `-o`.

Images run together on one thread pool sized to the machine. One image can use the whole pool. Several images share it, so the process does not start one full pool per file.

```text
crunch --help
crunch --version
```

Each finished file prints one line: percent of the original size, path, and byte count. When the result is not smaller, the original file is left in place and the line reports that size.

## For developers

Build requirements: a stable Rust toolchain and a C/C++ compiler (`clang`, `gcc`, or MSVC). pngdeer is built with its `parallel` and `oxipng` features only. WebP, JPEG, and AVIF are not compiled.

```bash
git clone --recurse-submodules https://github.com/you/crunch-cli.git
cd crunch-cli
git config core.hooksPath .githooks
cargo build --release
# binary: target/release/crunch
```

The repository pins two submodules:

- `third_party/pngdeer` at `612bfb8` (https://github.com/heydeer/pngdeer)
- `third_party/zopfli` at `df1517d` (https://github.com/google/zopfli)

`build.rs` compiles ZopfliPNG's library sources and links them into `crunch`. The Cargo root repeats pngdeer's `fast-ssim2` patch so scores match pngdeer's own binary. The submodule also contains vendored codecs that this build does not compile.

### Benchmark

`crunch-bench` is not in the release archive. It reads PNGs from the pngdeer submodule (`third_party/pngdeer/bench/images`, about 94 files; `bench/images/ui/` is not in that repository). It prints the original size, the size and time after pngdeer, the SSIMULACRA2 score and target, then the size and time after Zopfli. Animated files show `skipped` in the Zopfli column. Inputs are not modified.

```bash
cargo run --release --bin crunch-bench
cargo run --release --bin crunch-bench -- path/to/pngs
cargo run --release --bin crunch-bench -- --compare /path/to/old-crunch
```

`--compare` runs the previous Crunch binary (pngquant plus the older zopflipng) on a temporary copy. Pass that binary's path. Do not point it at the `crunch` built from this repository.

Continuous integration runs `crunch-bench` only on `third_party/pngdeer/bench/images/icons`, so the job stays within its time limit. Run the full tree locally.

### Checks

The pre-commit hook, after `git config core.hooksPath .githooks`, runs:

```bash
cargo fmt --check
cargo clippy --all-targets --no-deps -- -D warnings
```

Clippy lints this crate only. Warnings in pngdeer or oxipng are not treated as failures, and the hook does not format the submodules.

GitHub Actions on push and pull request runs those two checks, `cargo test`, `cargo build --release`, and the icon benchmark. A `v*` tag builds release archives and uploads them to the GitHub Release with `checksums.txt`:

- `x86_64-pc-windows-msvc`
- `x86_64-apple-darwin` and `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`

Archive names are `crunch-cli-<tag>-<target>`. A Homebrew formula can use those names and the checksum file later. This repository does not publish a tap.

pngdeer is MIT licensed. Zopfli is Apache-2.0. Test images under `third_party/pngdeer/bench/images` keep the licenses listed in that directory's `SOURCES.md`.
