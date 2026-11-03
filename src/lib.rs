//! Lossy PNG compression: a pngdeer palette search, then Zopfli on still images.

mod files;
mod pipeline;
mod zopfli;

pub use files::{commit, resolve_jobs, Job};
pub use pipeline::{optimize, optimize_with, Optimized, Palette, PngdeerPalette, ZopfliStage};
pub use zopfli::ZopfliPng;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Images shipped in the pngdeer submodule. Present after `git clone --recurse-submodules`.
pub const DEFAULT_BENCH_DIR: &str = "third_party/pngdeer/bench/images";
