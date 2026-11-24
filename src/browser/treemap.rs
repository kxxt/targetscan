use super::{
    Browser,
    size::{SizeScale, format_size},
};
use crate::chart;
use std::path::Path;
use tuirealm::ratatui::{
    Frame,
    layout::Rect,
    style::Stylize,
    widgets::{Block, BorderType, Paragraph},
};
impl Browser {
    pub(super) fn draw_treemap(
        &mut self,
        f: &mut Frame,
        root: &Path,
        tree: Rect,
        scale: SizeScale,
    ) {
        let paths = &self.scan.targets;
        let group_start = self.cursor / 8 * 8;
        let group_end = (group_start + 8).min(paths.len());
        let name_for = |p: &Path| {
            let name = p.strip_prefix(root).unwrap_or(p);
            if name.as_os_str().is_empty() {
                ".".to_string()
            } else {
                format!("{name:?}")
            }
        };
        // Include both borders and the widest selection marker in the budget.
        let label_width = paths[group_start..group_end]
            .iter()
            .map(|p| {
                tuirealm::ratatui::text::Line::from(name_for(p))
                    .width()
                    .saturating_add(6)
            })
            .max()
            .unwrap_or(16)
            .clamp(16, usize::from(tree.width)) as u16;
        let page_size =
            usize::from(tree.width / label_width) * usize::from(tree.height / chart::TILE_HEIGHT);
        self.page_start = group_start + (self.cursor - group_start) / page_size * page_size;
        let end = (self.page_start + page_size).min(group_end);
        let page = &paths[self.page_start..end];
        let weights: Vec<_> = page.iter().map(|p| self.scan.bytes[p]).collect();
        chart::tiles(tree, &weights, label_width, &mut self.tiles);
        for (i, ((path, bytes), tile)) in page.iter().zip(weights).zip(&self.tiles).enumerate() {
            let index = self.page_start + i;
            let focused = index == self.cursor;
            let border = if focused {
                BorderType::Double
            } else {
                BorderType::Plain
            };
            let mark = if self.selected.contains(path) {
                "[✓]"
            } else if focused {
                "▶"
            } else {
                "·"
            };
            let text = format!("{mark} {}\n  {}", name_for(path), format_size(bytes));
            f.render_widget(
                Paragraph::new(text)
                    .black()
                    .bg(scale.color(bytes))
                    .block(Block::bordered().border_type(border)),
                *tile,
            );
        }
    }
}
