//! ZopfliPNG through the C API in `third_party/zopfli`. The library returns its buffer
//! even when the result is larger; callers keep the previous bytes in that case.

use std::os::raw::{c_char, c_int, c_uchar};

use crate::pipeline::ZopfliStage;

/// `kStrategyPredefined`: keep the scanline filters already chosen (oxipng's).
const STRATEGY_PREDEFINED: c_int = 7;

const KEEP_CHUNKS: [*const c_char; 6] = [
    c"cICP".as_ptr(),
    c"iCCP".as_ptr(),
    c"cHRM".as_ptr(),
    c"gAMA".as_ptr(),
    c"sRGB".as_ptr(),
    c"pHYs".as_ptr(),
];

#[repr(C)]
struct CZopfliPngOptions {
    lossy_transparent: c_int,
    lossy_8bit: c_int,
    filter_strategies: *mut c_int,
    num_filter_strategies: c_int,
    auto_filter_strategy: c_int,
    keepchunks: *mut *mut c_char,
    num_keepchunks: c_int,
    use_zopfli: c_int,
    num_iterations: c_int,
    num_iterations_large: c_int,
    block_split_strategy: c_int,
}

extern "C" {
    fn CZopfliPNGSetDefaults(options: *mut CZopfliPngOptions);
    fn CZopfliPNGOptimize(
        origpng: *const c_uchar,
        origpng_size: usize,
        options: *const CZopfliPngOptions,
        verbose: c_int,
        resultpng: *mut *mut c_uchar,
        resultpng_size: *mut usize,
    ) -> c_int;
}

/// The linked ZopfliPNG library.
pub struct ZopfliPng;

impl ZopfliStage for ZopfliPng {
    fn optimize(&self, png: &[u8]) -> Result<Vec<u8>, String> {
        unsafe { optimize_ffi(png) }
    }
}

unsafe fn optimize_ffi(png: &[u8]) -> Result<Vec<u8>, String> {
    let mut options = std::mem::zeroed::<CZopfliPngOptions>();
    CZopfliPNGSetDefaults(&mut options);
    let mut strategy = [STRATEGY_PREDEFINED];
    options.filter_strategies = strategy.as_mut_ptr();
    options.num_filter_strategies = 1;
    options.auto_filter_strategy = 0;
    let mut keep: Vec<*mut c_char> = KEEP_CHUNKS.iter().map(|ptr| *ptr as *mut c_char).collect();
    options.keepchunks = keep.as_mut_ptr();
    options.num_keepchunks = keep.len() as c_int;

    let mut result = std::ptr::null_mut();
    let mut result_size = 0usize;
    let ret = CZopfliPNGOptimize(
        png.as_ptr(),
        png.len(),
        &options,
        0,
        &mut result,
        &mut result_size,
    );
    if ret != 0 {
        return Err(format!("zopflipng failed ({ret})"));
    }
    if result.is_null() {
        return Err("zopflipng returned no bytes".into());
    }
    let bytes = std::slice::from_raw_parts(result, result_size).to_vec();
    libc::free(result as *mut libc::c_void);
    Ok(bytes)
}
