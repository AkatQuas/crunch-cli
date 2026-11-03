//! pngdeer, then Zopfli on still images. Animated PNGs stop after pngdeer:
//! ZopfliPNG keeps only IHDR, PLTE, tRNS, IDAT, and IEND, which drops animation frames.

use std::time::Instant;

use pngdeer::Settings;

/// One image after both stages. `output` is the bytes to publish when they are smaller
/// than the input; otherwise the caller leaves the file alone.
pub struct Optimized {
    pub input_len: usize,
    pub pngdeer_len: usize,
    pub pngdeer_secs: f64,
    pub score: f64,
    pub target: f64,
    pub animated: bool,
    /// `None` when the image is animated and Zopfli was not run.
    pub zopfli_secs: Option<f64>,
    pub output: Vec<u8>,
}

pub trait Palette {
    fn compress(&self, input: &[u8], settings: &Settings) -> Result<PaletteOutput, String>;
}

pub struct PaletteOutput {
    pub png: Vec<u8>,
    pub score: f64,
    pub target: f64,
    pub secs: f64,
}

pub trait ZopfliStage {
    fn optimize(&self, png: &[u8]) -> Result<Vec<u8>, String>;
}

/// pngdeer's search and its oxipng pass.
pub struct PngdeerPalette;

impl Palette for PngdeerPalette {
    fn compress(&self, input: &[u8], settings: &Settings) -> Result<PaletteOutput, String> {
        let started = Instant::now();
        let compressed = pngdeer::compress(input, settings).map_err(|err| err.to_string())?;
        Ok(PaletteOutput {
            png: compressed.png,
            score: compressed.report.score,
            target: compressed.report.target,
            secs: started.elapsed().as_secs_f64(),
        })
    }
}

pub fn optimize_with(
    input: &[u8],
    settings: &Settings,
    palette: &impl Palette,
    zopfli: &impl ZopfliStage,
) -> Result<Optimized, String> {
    let animated = pngdeer::apng::is_animated(input);
    let searched = palette.compress(input, settings)?;
    if animated {
        return Ok(Optimized {
            input_len: input.len(),
            pngdeer_len: searched.png.len(),
            pngdeer_secs: searched.secs,
            score: searched.score,
            target: searched.target,
            animated: true,
            zopfli_secs: None,
            output: searched.png,
        });
    }
    let started = Instant::now();
    let zopfli_png = zopfli.optimize(&searched.png)?;
    let zopfli_secs = started.elapsed().as_secs_f64();
    let output = if zopfli_png.len() < searched.png.len() {
        zopfli_png
    } else {
        searched.png.clone()
    };
    Ok(Optimized {
        input_len: input.len(),
        pngdeer_len: searched.png.len(),
        pngdeer_secs: searched.secs,
        score: searched.score,
        target: searched.target,
        animated: false,
        zopfli_secs: Some(zopfli_secs),
        output,
    })
}

pub fn optimize(input: &[u8], settings: &Settings) -> Result<Optimized, String> {
    optimize_with(input, settings, &PngdeerPalette, &crate::ZopfliPng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FixedPalette {
        png: Vec<u8>,
    }

    impl Palette for FixedPalette {
        fn compress(&self, _input: &[u8], _settings: &Settings) -> Result<PaletteOutput, String> {
            Ok(PaletteOutput {
                png: self.png.clone(),
                score: 90.0,
                target: 85.0,
                secs: 0.0,
            })
        }
    }

    struct RecordingZopfli {
        png: Vec<u8>,
        calls: AtomicUsize,
    }

    impl ZopfliStage for RecordingZopfli {
        fn optimize(&self, _png: &[u8]) -> Result<Vec<u8>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.png.clone())
        }
    }

    fn still_png() -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&[0, 0, 0, 0]);
        png.extend_from_slice(b"IEND");
        png
    }

    fn animated_png() -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&[0, 0, 0, 8]);
        png.extend_from_slice(b"acTL");
        png.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 0]);
        png.extend_from_slice(&[0, 0, 0, 0]);
        png.extend_from_slice(&[0, 0, 0, 0]);
        png.extend_from_slice(b"IDAT");
        png
    }

    #[test]
    fn keeps_zopfli_bytes_only_when_smaller() {
        let input = still_png();
        let smaller = RecordingZopfli {
            png: b"small".to_vec(),
            calls: AtomicUsize::new(0),
        };
        let out = optimize_with(
            &input,
            &Settings::default(),
            &FixedPalette {
                png: b"palette-bytes".to_vec(),
            },
            &smaller,
        )
        .unwrap();
        assert_eq!(out.output, b"small");
        assert_eq!(smaller.calls.load(Ordering::SeqCst), 1);
        assert!(!out.animated);

        let larger = RecordingZopfli {
            png: b"palette-bytes-and-more".to_vec(),
            calls: AtomicUsize::new(0),
        };
        let out = optimize_with(
            &input,
            &Settings::default(),
            &FixedPalette {
                png: b"palette-bytes".to_vec(),
            },
            &larger,
        )
        .unwrap();
        assert_eq!(out.output, b"palette-bytes");
        assert_eq!(larger.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn animated_input_does_not_call_zopfli() {
        let zopfli = RecordingZopfli {
            png: b"nope".to_vec(),
            calls: AtomicUsize::new(0),
        };
        let out = optimize_with(
            &animated_png(),
            &Settings::default(),
            &FixedPalette {
                png: b"frames".to_vec(),
            },
            &zopfli,
        )
        .unwrap();
        assert!(out.animated);
        assert_eq!(out.output, b"frames");
        assert!(out.zopfli_secs.is_none());
        assert_eq!(zopfli.calls.load(Ordering::SeqCst), 0);
    }
}
