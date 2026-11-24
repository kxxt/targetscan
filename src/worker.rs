use color_eyre::Result;
use std::{
    path::{Path, PathBuf},
    thread,
};
use targetscan::{Error, Scan, delete_output, scan};

trait CanManageTargets {
    fn scan(&self, root: &Path) -> Result<Scan, Error>;
    fn remove(&self, root: &Path, path: &Path) -> Result<(), Error>;
}

struct LocalFilesystem;

pub(crate) fn work(
    root: PathBuf,
    paths: Vec<PathBuf>,
) -> thread::JoinHandle<Result<(Scan, String)>> {
    thread::spawn(move || run(&LocalFilesystem, &root, paths))
}

fn run<C>(_context: &C, root: &Path, paths: Vec<PathBuf>) -> Result<(Scan, String)> {
    let mut deleted = 0;
    let mut errors = Vec::new();
    for path in paths {
        match delete_output(root, &path) {
            Ok(()) if path == root => {
                return Ok((
                    Scan::default(),
                    "Deleted the scanned target directory".into(),
                ));
            }
            Ok(()) => deleted += 1,
            Err(e) => errors.push(e.to_string()),
        }
    }
    let scan = scan(root)?;
    let mut status = format!("{} found · {} skipped", scan.targets.len(), scan.skipped);
    if deleted > 0 {
        status.push_str(&format!(" · {deleted} deleted"));
    }
    if let Some(error) = errors.first() {
        status = format!("{} failed · {error} · {status}", errors.len());
    }
    Ok((scan, status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_that::prelude::*;

    struct PartialFailure;

    impl CanManageTargets for PartialFailure {
        fn remove(&self, root: &Path, path: &Path) -> Result<(), Error> {
            if path == root.join("changed") {
                Err(Error::Changed {
                    root: root.into(),
                    path: path.into(),
                })
            } else {
                Ok(())
            }
        }

        fn scan(&self, root: &Path) -> Result<Scan, Error> {
            Ok(Scan {
                targets: vec![root.join("changed")],
                ..Scan::default()
            })
        }
    }

    #[test]
    fn worker_uses_its_context_to_continue_after_a_failed_deletion() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path();
        let paths = vec![root.join("changed"), root.join("removable")];
        let (scan, status) = run(&PartialFailure, root, paths)?;
        assert_that!(scan.targets, eq(vec![root.join("changed")]));
        assert_that!(status.starts_with("1 failed"), eq(true));
        assert_that!(status.ends_with("1 deleted"), eq(true));
        Ok(())
    }
}
