use crossterm::event::KeyCode;
use tuirealm::ratatui::{Frame, layout::Rect, style::Stylize, text::Span};

use super::Browser;

struct Shortcut(&'static str, &'static str, KeyCode);

impl Browser {
    /// Lay out in footer-local coordinates before reserving the footer's rows.
    pub(super) fn footer_layout(
        &self,
        width: u16,
        busy: bool,
    ) -> Vec<(Rect, Span<'static>, KeyCode)> {
        use KeyCode::{Char, Down, Esc, Up};
        let mut actions = Vec::new();
        if self.dialog.pending().is_some() {
            actions.extend([
                Shortcut("Y confirm", "confirm", Char('y')),
                Shortcut("Esc / N cancel", "cancel", Esc),
            ]);
        }
        let browsing = self.dialog.pending().is_none();
        if !self.scan.targets.is_empty() {
            actions.extend([Shortcut("↑ prev", "↑", Up), Shortcut("↓ next", "↓", Down)]);
            if browsing {
                actions.push(Shortcut("Space mark", "mark", Char(' ')));
                actions.push(Shortcut("A select all", "all", Char('a')));
            }
        }
        if browsing {
            actions.push(Shortcut("T tree/map", "tree/map", Char('t')));
            if !busy && !self.scan.targets.is_empty() {
                actions.push(Shortcut("D delete", "delete", Char('d')));
            }
            actions.push(Shortcut("Q quit", "quit", Char('q')));
        }

        let full_width = actions
            .iter()
            .map(|a| Span::raw(a.0).width())
            .sum::<usize>()
            + actions.len().saturating_sub(1) * 2;
        let compact = full_width > usize::from(width);
        let (mut x, mut y): (u16, u16) = (0, 0);
        actions
            .into_iter()
            .map(|Shortcut(full, short, key)| {
                let label = Span::raw(if compact { short } else { full }).underlined();
                let len = (label.width() as u16).min(width);
                if x > 0 && x.saturating_add(len) > width {
                    x = 0;
                    y += 1;
                }
                let rect = Rect::new(x, y, len, 1);
                x = rect.right().saturating_add(2);
                (rect, label, key)
            })
            .collect()
    }

    pub(super) fn draw_shortcuts(
        &mut self,
        f: &mut Frame,
        area: Rect,
        shortcuts: Vec<(Rect, Span<'static>, KeyCode)>,
    ) {
        for (rect, label, key) in shortcuts {
            let rect = Rect::new(
                area.x + rect.x,
                area.y.saturating_add(rect.y),
                rect.width,
                1,
            )
            .intersection(area);
            if !rect.is_empty() {
                f.render_widget(label, rect);
                self.buttons.push((rect, key));
            }
        }
    }
}
