//! Which files to compress, and where the bytes go.
//!
//! `-o` / `--output` applies only when the invocation names exactly one file.
//! Several files, or a directory, are always replaced in place. A result that is
//! not smaller than the input is not written.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub input: PathBuf,
    /// `Some` writes a new file and leaves `input` untouched. `None` replaces `input`.
    pub output: Option<PathBuf>,
}

pub fn resolve_jobs(paths: &[PathBuf], output: Option<&Path>) -> Result<Vec<Job>, String> {
    if paths.is_empty() {
        return Err("pass at least one PNG file or directory".into());
    }
    let single_file = paths.len() == 1 && paths[0].is_file();
    let explicit = if single_file { output } else { None };
    let mut jobs = Vec::new();
    for path in paths {
        if path.is_dir() {
            collect_dir(path, &mut jobs)?;
        } else if path.is_file() {
            if !is_png(path) {
                return Err(format!("{} is not a PNG file", path.display()));
            }
            jobs.push(Job {
                input: path.clone(),
                output: explicit.map(Path::to_path_buf),
            });
        } else {
            return Err(format!("no such file or directory: {}", path.display()));
        }
    }
    if jobs.is_empty() {
        return Err("no PNG files found".into());
    }
    jobs.sort_by(|a, b| a.input.cmp(&b.input));
    Ok(jobs)
}

fn collect_dir(dir: &Path, jobs: &mut Vec<Job>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("{}: {err}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_dir(&path, jobs)?;
        } else if path.is_file() && is_png(&path) {
            jobs.push(Job {
                input: path,
                output: None,
            });
        }
    }
    Ok(())
}

fn is_png(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
}

/// Writes `bytes` only when they are shorter than the file at `input`.
/// Returns whether a file was written.
pub fn commit(input: &Path, output: Option<&Path>, bytes: &[u8]) -> Result<bool, String> {
    let original = fs::metadata(input)
        .map_err(|err| format!("{}: {err}", input.display()))?
        .len() as usize;
    if bytes.len() >= original {
        return Ok(false);
    }
    let destination = output.unwrap_or(input);
    let tmp = destination.with_extension("png.crunch-tmp");
    fs::write(&tmp, bytes).map_err(|err| format!("{}: {err}", tmp.display()))?;
    fs::rename(&tmp, destination).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        format!("{}: {err}", destination.display())
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn output_flag_only_for_one_file() {
        let dir = std::env::temp_dir().join(format!("crunch-cli-jobs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("nested")).unwrap();
        touch(&dir.join("a.png"), b"aaa");
        touch(&dir.join("b.png"), b"bbb");
        touch(&dir.join("nested").join("c.png"), b"ccc");
        touch(&dir.join("skip.txt"), b"no");
        let out = dir.join("out.png");

        let one = resolve_jobs(&[dir.join("a.png")], Some(&out)).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].output.as_deref(), Some(out.as_path()));

        let many = resolve_jobs(&[dir.join("a.png"), dir.join("b.png")], Some(&out)).unwrap();
        assert!(many.iter().all(|job| job.output.is_none()));
        assert_eq!(many.len(), 2);

        let tree = resolve_jobs(std::slice::from_ref(&dir), Some(&out)).unwrap();
        assert!(tree.iter().all(|job| job.output.is_none()));
        assert_eq!(
            tree.iter().map(|job| job.input.clone()).collect::<Vec<_>>(),
            vec![
                dir.join("a.png"),
                dir.join("b.png"),
                dir.join("nested").join("c.png"),
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_skips_a_result_that_is_not_smaller() {
        let dir = std::env::temp_dir().join(format!("crunch-cli-commit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.png");
        touch(&src, b"12345");
        assert!(!commit(&src, None, b"12345").unwrap());
        assert_eq!(fs::read(&src).unwrap(), b"12345");
        assert!(!commit(&src, None, b"123456").unwrap());
        assert_eq!(fs::read(&src).unwrap(), b"12345");

        assert!(commit(&src, None, b"12").unwrap());
        assert_eq!(fs::read(&src).unwrap(), b"12");

        touch(&src, b"12345");
        let out = dir.join("out.png");
        assert!(!commit(&src, Some(&out), b"12345").unwrap());
        assert!(!out.exists());
        assert!(commit(&src, Some(&out), b"1").unwrap());
        assert_eq!(fs::read(&src).unwrap(), b"12345");
        assert_eq!(fs::read(&out).unwrap(), b"1");
        let _ = fs::remove_dir_all(&dir);
    }
}
