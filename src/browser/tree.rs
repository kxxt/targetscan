use super::{
    Browser,
    size::{SizeScale, format_size},
};
use std::path::Path;
use tuirealm::ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    widgets::{List, ListItem},
};
impl Browser {
    pub(super) fn draw_tree(&mut self, f: &mut Frame, root: &Path, tree: Rect, scale: SizeScale) {
        let pending = self.dialog.pending();
        let paths = pending.unwrap_or(&self.scan.targets);
        let largest = scale.largest;
        let rows = paths.iter().enumerate().map(|(i, path)| {
            let branch = if pending.is_none() && i == self.cursor {
                "▶─"
            } else if i + 1 == paths.len() {
                "└─"
            } else {
                "├─"
            };
            let mark = if pending.is_some() {
                "×"
            } else if self.selected.contains(path) {
                "[✓]"
            } else {
                "[ ]"
            };
            let relative = path.strip_prefix(root).unwrap_or(path);
            let name = if relative.as_os_str().is_empty() {
                Path::new(".")
            } else {
                relative
            };
            let bytes = self.scan.bytes[path];
            ListItem::new(format!("{branch} {mark} {name:?}  {}", format_size(bytes)))
                .fg(scale.color(bytes))
        });
        self.list.select((!paths.is_empty()).then_some(self.cursor));
        f.render_stateful_widget(
            List::new(rows).highlight_style(Style::default().bg(Color::DarkGray).bold()),
            tree,
            &mut self.list,
        );
        if pending.is_none() && largest > 0 {
            // List rendering updates its offset to keep the focused entry visible.
            let visible = paths.iter().skip(self.list.offset()).zip(tree.rows());
            for (path, row) in visible {
                let bytes = self.scan.bytes[path];
                let width = (u128::from(row.width) * u128::from(bytes))
                    .div_ceil(u128::from(largest)) as u16;
                let style = Style::default().black().bg(scale.color(bytes));
                f.buffer_mut().set_style(Rect { width, ..row }, style);
            }
        }
    }
}
