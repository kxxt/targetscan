use snafu::{ResultExt, Snafu};
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Default)]
pub struct Scan {
    pub targets: Vec<PathBuf>,
    pub skipped: usize,
    pub bytes: std::collections::BTreeMap<PathBuf, u64>,
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Failed to open scan directory {path:?}: {source}"))]
    Open { path: PathBuf, source: io::Error },
    #[snafu(display("Directory {path:?} no longer qualifies as a Cargo output under {root:?}"))]
    Changed { path: PathBuf, root: PathBuf },
    #[snafu(display("Failed to delete Cargo output {path:?}: {source}"))]
    Delete { path: PathBuf, source: io::Error },
}

fn is_output(path: &Path) -> bool {
    !path.join("Cargo.toml").exists()
        && (path.join(".rustc_info.json").is_file()
            || (path.file_name().is_some_and(|name| name == "target")
                && path
                    .parent()
                    .is_some_and(|parent| parent.join("Cargo.toml").is_file())))
}

pub fn scan(root: &Path) -> Result<Scan, Error> {
    let root = root.canonicalize().context(OpenSnafu { path: root })?;
    fs::read_dir(&root).context(OpenSnafu { path: &root })?;
    let mut result = Scan::default();
    let mut entries = walkdir::WalkDir::new(root).into_iter();
    while let Some(entry) = entries.next() {
        match entry {
            Ok(entry) if entry.file_type().is_dir() && is_output(entry.path()) => {
                entries.skip_current_dir();
                let path = entry.into_path();
                let mut bytes = 0u64;
                for child in walkdir::WalkDir::new(&path).follow_root_links(false) {
                    match child {
                        Ok(file) if file.file_type().is_file() => match file.metadata() {
                            Ok(info) => bytes = bytes.saturating_add(info.len()),
                            Err(_) => result.skipped += 1,
                        },
                        Err(_) => result.skipped += 1,
                        _ => {}
                    }
                }
                result.bytes.insert(path.clone(), bytes);
                result.targets.push(path);
            }
            Err(_) => result.skipped += 1,
            _ => {}
        }
    }
    result.targets.sort();
    Ok(result)
}

pub fn delete_output(root: &Path, path: &Path) -> Result<(), Error> {
    if !path.starts_with(root)
        || path.canonicalize().ok().as_deref() != Some(path)
        || !is_output(path)
    {
        return ChangedSnafu { path, root }.fail();
    }
    fs::remove_dir_all(path).context(DeleteSnafu { path })
}

#[derive(Default)]
pub struct Confirmation(Option<Vec<PathBuf>>);

impl Confirmation {
    pub fn request(&mut self, paths: Vec<PathBuf>) {
        self.0 = Some(paths);
    }
    pub fn pending(&self) -> Option<&[PathBuf]> {
        self.0.as_deref()
    }
    pub fn resolve(&mut self, approved: bool) -> Vec<PathBuf> {
        self.0.take().filter(|_| approved).unwrap_or_default()
    }
}
