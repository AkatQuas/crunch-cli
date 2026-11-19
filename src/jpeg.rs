//! JPEG in-place optimization via slimdeer (re-encode at the lowest quality that keeps the target).

use slimdeer::Settings;

pub fn optimize_jpeg(input: &[u8], settings: &Settings) -> Result<Vec<u8>, String> {
    slimdeer::compress_jpeg(input, settings)
        .map(|done| done.jpeg)
        .map_err(|err| err.to_string())
}
