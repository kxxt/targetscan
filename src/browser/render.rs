use super::{
    Browser,
    size::{SizeScale, format_size, gradient},
};
use crate::chart;
use std::path::Path;
use tuirealm::ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
impl Browser {
    pub(super) fn draw(&mut self, f: &mut Frame, root: &Path, busy: Option<&str>, tick: usize) {
        self.buttons.clear();
        let shortcuts = self.footer_layout(f.area().width, busy.is_some());
        let footer_height = shortcuts.last().map_or(1, |(rect, _, _)| rect.bottom());
        let [header, directory, tree, status, actions, keys] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(footer_height),
        ])
        .areas(f.area());
        let pending = self.dialog.pending();
        let title = if pending.is_some() {
            "Confirm permanent deletion"
        } else if !self.treemap_view {
            "Cargo target tree · bar length by file size"
        } else {
            "Cargo target treemap · area by file size"
        };
        f.render_widget(Paragraph::new(title).bold().cyan(), header);
        f.render_widget(Paragraph::new(format!("{:?}", root)).dark_gray(), directory);
        let [tree, rail] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(tree);
        let scale = SizeScale::new(self.scan.bytes.values().copied());
        let SizeScale { smallest, largest } = scale;
        self.tiles.clear();
        if pending.is_none()
            && self.treemap_view
            && !self.scan.targets.is_empty()
            && tree.height >= chart::TILE_HEIGHT
            && tree.width >= 16
        {
            self.draw_treemap(f, root, tree, scale);
        } else {
            self.draw_tree(f, root, tree, scale);
        }
        let pending = self.dialog.pending();
        let paths = pending.unwrap_or(&self.scan.targets);
        if !paths.is_empty() {
            let (offset, visible) = if self.tiles.is_empty() {
                (
                    self.list.offset(),
                    usize::from(tree.height).min(paths.len()),
                )
            } else {
                (self.page_start, self.tiles.len())
            };
            let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_symbol("█")
                .track_symbol(Some("│"));
            let mut state = ScrollbarState::new(paths.len() - visible + 1)
                .viewport_content_length(visible)
                .position(offset);
            f.render_stateful_widget(bar, rail, &mut state);
        }
        let label = if let Some(paths) = pending {
            format!(
                "Permanently remove {} director{} and all contents?",
                paths.len(),
                if paths.len() == 1 { "y" } else { "ies" }
            )
        } else if let Some(task) = busy {
            let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            format!("{} {task}", spinner[tick % spinner.len()])
        } else if self.treemap_view
            && let Some(path) = paths.get(self.cursor)
        {
            format!(
                "{} · {}/{} {:?} ({})",
                self.status,
                self.cursor + 1,
                paths.len(),
                path.strip_prefix(root).unwrap_or(path),
                format_size(self.scan.bytes[path])
            )
        } else {
            self.status.clone()
        };
        f.render_widget(Paragraph::new(label).yellow(), status);
        if pending.is_none() && !paths.is_empty() {
            let [button, legend] =
                Layout::horizontal([Constraint::Length(13), Constraint::Min(0)]).areas(actions);
            let mut scale = vec![Span::raw(format!("{} ", format_size(smallest)))];
            if smallest == largest {
                scale.push(Span::styled("■", gradient(0.5)));
            } else {
                let last = format!(" {}", format_size(largest));
                let labels = scale[0].width() + last.len();
                let steps = usize::from(legend.width)
                    .saturating_sub(labels)
                    .clamp(2, 24);
                scale.extend(
                    (0..steps).map(|i| Span::styled("█", gradient(i as f64 / (steps - 1) as f64))),
                );
                scale.push(Span::raw(last));
            }
            f.render_widget(Line::from(scale), legend);
            if busy.is_none() {
                let button = Rect {
                    width: button.width.min(12),
                    ..button
                };
                f.render_widget(
                    Paragraph::new(" [D] Delete ").black().on_light_red(),
                    button,
                );
                self.buttons
                    .push((button, crossterm::event::KeyCode::Char('d')));
            }
        }
        self.draw_shortcuts(f, keys, shortcuts);
    }
}
