//! Lossy PNG and JPEG compression: slimdeer palette search plus Zopfli on still PNGs;
//! JPEGs are re-encoded in place through slimdeer only.

mod files;
mod jpeg;
mod pipeline;
mod zopfli;

pub use files::{commit, resolve_jobs, ImageKind, Job};
pub use jpeg::optimize_jpeg;
pub use pipeline::{optimize, optimize_with, Optimized, Palette, SlimdeerPalette, ZopfliStage};
pub use zopfli::ZopfliPng;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Images shipped in the slimdeer submodule. Present after `git clone --recurse-submodules`.
pub const DEFAULT_BENCH_DIR: &str = "third_party/slimdeer/bench/images";
