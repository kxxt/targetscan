use std::io;
use std::path::{Path, PathBuf};

pub struct Scan {
    pub targets: Vec<PathBuf>,
    pub skipped: usize,
}

pub fn scan(_root: &Path) -> io::Result<Scan> {
    Ok(Scan {
        targets: vec![],
        skipped: 0,
    })
}

#[derive(Default)]
pub struct Confirmation(Option<Vec<PathBuf>>);

impl Confirmation {
    pub fn request(&mut self, _paths: Vec<PathBuf>) {}
    pub fn pending(&self) -> Option<&[PathBuf]> {
        self.0.as_deref()
    }
    pub fn resolve(&mut self, _approved: bool) -> Vec<PathBuf> {
        Vec::new()
    }
}
