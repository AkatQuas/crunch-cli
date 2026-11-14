//! slimdeer, then Zopfli on still images. Animated PNGs stop after slimdeer:
//! ZopfliPNG keeps only IHDR, PLTE, tRNS, IDAT, and IEND, which drops animation frames.
//! Both stages drop `eXIf`; the original chunk is copied back onto the bytes we keep.

use std::time::Instant;

use slimdeer::Settings;

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

/// slimdeer's search and its oxipng pass.
pub struct SlimdeerPalette;

impl Palette for SlimdeerPalette {
    fn compress(&self, input: &[u8], settings: &Settings) -> Result<PaletteOutput, String> {
        let started = Instant::now();
        let compressed = slimdeer::compress(input, settings).map_err(|err| err.to_string())?;
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
    let animated = slimdeer::apng::is_animated(input);
    let searched = palette.compress(input, settings)?;
    let pngdeer_len = searched.png.len();
    let pngdeer_secs = searched.secs;
    let score = searched.score;
    let target = searched.target;
    let (output, zopfli_secs) = if animated {
        (searched.png, None)
    } else {
        let started = Instant::now();
        let zopfli_png = zopfli.optimize(&searched.png)?;
        let zopfli_secs = started.elapsed().as_secs_f64();
        let output = if zopfli_png.len() < searched.png.len() {
            zopfli_png
        } else {
            searched.png
        };
        (output, Some(zopfli_secs))
    };
    Ok(Optimized {
        input_len: input.len(),
        pngdeer_len,
        pngdeer_secs,
        score,
        target,
        animated,
        zopfli_secs,
        output: restore_exif(output, input),
    })
}

pub fn optimize(input: &[u8], settings: &Settings) -> Result<Optimized, String> {
    optimize_with(input, settings, &SlimdeerPalette, &crate::ZopfliPng)
}

/// One PNG chunk that parsed completely: length, type, data, and CRC.
struct PngChunk<'a> {
    at: usize,
    raw: &'a [u8],
    kind: [u8; 4],
}

fn png_chunks(png: &[u8]) -> Vec<PngChunk<'_>> {
    let mut chunks = Vec::new();
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return chunks;
    }
    let mut pos = 8usize;
    while pos.saturating_add(12) <= png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
        let Some(end) = pos.checked_add(12).and_then(|start| start.checked_add(len)) else {
            break;
        };
        if end > png.len() {
            break;
        }
        let kind = png[pos + 4..pos + 8].try_into().unwrap();
        chunks.push(PngChunk {
            at: pos,
            raw: &png[pos..end],
            kind,
        });
        pos = end;
    }
    chunks
}

fn chunk_data(chunk: &[u8]) -> &[u8] {
    let len = u32::from_be_bytes(chunk[..4].try_into().unwrap()) as usize;
    &chunk[8..8 + len]
}

/// `eXIf` chunks from `source`, copied in front of the first `IDAT` (or `IEND`) of `output`
/// when that payload is not already there. Anything that is not a PNG is left as it is.
fn restore_exif(output: Vec<u8>, source: &[u8]) -> Vec<u8> {
    let wanted: Vec<Vec<u8>> = png_chunks(source)
        .into_iter()
        .filter(|chunk| &chunk.kind == b"eXIf")
        .map(|chunk| chunk.raw.to_vec())
        .collect();
    if wanted.is_empty() {
        return output;
    }
    let have = png_chunks(&output);
    let missing: Vec<&[u8]> = wanted
        .iter()
        .filter(|chunk| {
            let data = chunk_data(chunk);
            !have
                .iter()
                .any(|existing| &existing.kind == b"eXIf" && chunk_data(existing.raw) == data)
        })
        .map(Vec::as_slice)
        .collect();
    if missing.is_empty() {
        return output;
    }
    let Some(at) = have
        .iter()
        .find(|chunk| &chunk.kind == b"IDAT")
        .or_else(|| have.iter().find(|chunk| &chunk.kind == b"IEND"))
        .map(|chunk| chunk.at)
    else {
        return output;
    };
    let extra: usize = missing.iter().map(|chunk| chunk.len()).sum();
    let mut png = Vec::with_capacity(output.len() + extra);
    png.extend_from_slice(&output[..at]);
    for chunk in missing {
        png.extend_from_slice(chunk);
    }
    png.extend_from_slice(&output[at..]);
    png
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

    fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut chunk = Vec::with_capacity(12 + data.len());
        chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
        chunk.extend_from_slice(kind);
        chunk.extend_from_slice(data);
        chunk.extend_from_slice(&0u32.to_be_bytes());
        chunk
    }

    fn png_file(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        for chunk in chunks {
            png.extend_from_slice(chunk);
        }
        png
    }

    #[test]
    fn restores_exif_before_the_first_idat() {
        let ihdr = png_chunk(b"IHDR", &[0; 13]);
        let exif = png_chunk(b"eXIf", b"II*\0camera");
        let plte = png_chunk(b"PLTE", &[1, 2, 3]);
        let idat = png_chunk(b"IDAT", b"pixels");
        let iend = png_chunk(b"IEND", b"");
        let input = png_file(&[
            ihdr.clone(),
            exif.clone(),
            plte.clone(),
            idat.clone(),
            iend.clone(),
        ]);
        let stripped = png_file(&[ihdr.clone(), plte, idat, iend.clone()]);
        let zopfli_idat = png_chunk(b"IDAT", b"z");
        let zopfli = RecordingZopfli {
            png: png_file(&[ihdr.clone(), zopfli_idat.clone(), iend.clone()]),
            calls: AtomicUsize::new(0),
        };
        let out = optimize_with(
            &input,
            &Settings::default(),
            &FixedPalette { png: stripped },
            &zopfli,
        )
        .unwrap();
        assert_eq!(out.output, png_file(&[ihdr, exif, zopfli_idat, iend]));
        assert_eq!(zopfli.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn does_not_duplicate_an_exif_chunk_already_present() {
        let file = png_file(&[
            png_chunk(b"IHDR", &[0; 13]),
            png_chunk(b"eXIf", b"II*\0camera"),
            png_chunk(b"IDAT", b"pixels"),
            png_chunk(b"IEND", b""),
        ]);
        let zopfli = RecordingZopfli {
            png: file.clone(),
            calls: AtomicUsize::new(0),
        };
        let out = optimize_with(
            &file,
            &Settings::default(),
            &FixedPalette { png: file.clone() },
            &zopfli,
        )
        .unwrap();
        assert_eq!(out.output, file);
    }

    #[test]
    fn animated_output_gets_exif_without_zopfli() {
        let ihdr = png_chunk(b"IHDR", &[0; 13]);
        let actl = png_chunk(b"acTL", &[0; 8]);
        let exif = png_chunk(b"eXIf", b"II*\0camera");
        let idat = png_chunk(b"IDAT", b"frame");
        let iend = png_chunk(b"IEND", b"");
        let input = png_file(&[
            ihdr.clone(),
            actl.clone(),
            exif.clone(),
            idat.clone(),
            iend.clone(),
        ]);
        let stripped = png_file(&[ihdr.clone(), actl.clone(), idat.clone(), iend.clone()]);
        let zopfli = RecordingZopfli {
            png: b"nope".to_vec(),
            calls: AtomicUsize::new(0),
        };
        let out = optimize_with(
            &input,
            &Settings::default(),
            &FixedPalette { png: stripped },
            &zopfli,
        )
        .unwrap();
        assert!(out.animated);
        assert!(out.zopfli_secs.is_none());
        assert_eq!(zopfli.calls.load(Ordering::SeqCst), 0);
        assert_eq!(out.output, png_file(&[ihdr, actl, exif, idat, iend]));
    }

    #[test]
    fn exif_lands_before_iend_when_there_is_no_idat() {
        let ihdr = png_chunk(b"IHDR", &[0; 13]);
        let exif = png_chunk(b"eXIf", b"II*\0camera");
        let iend = png_chunk(b"IEND", b"");
        let source = png_file(&[ihdr.clone(), exif.clone(), iend.clone()]);
        let output = png_file(&[ihdr.clone(), iend.clone()]);
        assert_eq!(restore_exif(output, &source), png_file(&[ihdr, exif, iend]));
    }
}
