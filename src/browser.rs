mod footer;
mod render;
mod session;
mod size;
#[cfg(test)]
mod tests;
mod tree;
mod treemap;

pub(crate) use session::run;

use crossterm::event::KeyCode;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use targetscan::{Confirmation, Scan};
use tuirealm::ratatui::{
    layout::{Position, Rect},
    widgets::ListState,
};

#[derive(Default)]
struct Browser {
    scan: Scan,
    cursor: usize,
    selected: BTreeSet<PathBuf>,
    dialog: Confirmation,
    browse_cursor: usize,
    status: String,
    list: ListState,
    buttons: Vec<(Rect, KeyCode)>,
    treemap_view: bool,
    tiles: Vec<Rect>,
    page_start: usize,
}

impl Browser {
    fn footer_key(&self, position: Position) -> Option<KeyCode> {
        self.buttons
            .iter()
            .find(|(rect, _)| rect.contains(position))
            .map(|(_, key)| *key)
    }

    fn toggle_view(&mut self) {
        let focused = self.scan.targets.get(self.cursor).cloned();
        self.treemap_view = !self.treemap_view;
        self.order_targets(focused.as_deref());
    }

    fn order_targets(&mut self, focused: Option<&Path>) {
        let Scan { targets, bytes, .. } = &mut self.scan;
        if !self.treemap_view {
            targets.sort_unstable_by(|a, b| bytes[b].cmp(&bytes[a]).then_with(|| a.cmp(b)));
        } else {
            targets.sort_unstable();
        }
        self.cursor = focused
            .and_then(|path| targets.iter().position(|entry| entry == path))
            .unwrap_or_else(|| self.cursor.min(targets.len().saturating_sub(1)));
    }

    fn refresh(&mut self, scan: Scan, status: String) {
        let focused = self.scan.targets.get(self.cursor).cloned();
        self.selected.retain(|path| scan.bytes.contains_key(path));
        self.scan = scan;
        self.order_targets(focused.as_deref());
        self.status = status;
    }

    fn request(&mut self) {
        let paths: Vec<PathBuf> = if self.selected.is_empty() {
            self.scan
                .targets
                .get(self.cursor)
                .cloned()
                .into_iter()
                .collect()
        } else {
            self.selected.iter().cloned().collect()
        };
        if !paths.is_empty() {
            self.browse_cursor = self.cursor;
            self.dialog.request(paths);
            self.cursor = 0;
        }
    }
}
