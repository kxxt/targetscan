use color_eyre::Result;
use std::{path::PathBuf, thread};
use targetscan::{Scan, delete_output, scan};
pub(crate) fn work(
    root: PathBuf,
    paths: Vec<PathBuf>,
) -> thread::JoinHandle<Result<(Scan, String)>> {
    thread::spawn(move || {
        let mut deleted = 0;
        let mut errors = Vec::new();
        for path in paths {
            match delete_output(&root, &path) {
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
        let scan = scan(&root)?;
        let mut status = format!("{} found · {} skipped", scan.targets.len(), scan.skipped);
        if deleted > 0 {
            status.push_str(&format!(" · {deleted} deleted"));
        }
        if let Some(error) = errors.first() {
            status = format!("{} failed · {error} · {status}", errors.len());
        }
        Ok((scan, status))
    })
}
