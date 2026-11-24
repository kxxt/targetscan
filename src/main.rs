mod chart;
use clap::Parser;
use color_eyre::eyre::{Result, WrapErr, eyre};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use targetscan::{Confirmation, Scan, delete_output, scan};
use tuirealm::{
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Position, Rect},
        style::{Color, Stylize},
        text::{Line, Span},
        widgets::{
            Block, BorderType, List, ListItem, ListState, Paragraph, Scrollbar,
            ScrollbarOrientation, ScrollbarState,
        },
    },
    terminal::{CrosstermTerminalAdapter, TerminalAdapter},
};

const MIB: u64 = 1 << 20;
const SIZE_BANDS: [(u64, Color, &str); 4] = [
    (0, Color::Cyan, "<1 MiB"),
    (MIB, Color::Green, "1 MiB+"),
    (100 * MIB, Color::Yellow, "100 MiB+"),
    (1024 * MIB, Color::LightRed, "1 GiB+"),
];

fn size_color(bytes: u64) -> Color {
    let band = SIZE_BANDS.partition_point(|&(minimum, _, _)| minimum <= bytes);
    SIZE_BANDS[band - 1].1
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    for unit in ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] {
        value /= 1024.0;
        if value < 1024.0 {
            return format!("{value:.1} {unit}");
        }
    }
    unreachable!("all u64 byte counts fit in EiB")
}

#[derive(Parser)]
#[command(
    version,
    about = "Explore Cargo build directories and remove selected outputs"
)]
struct Args {
    /// Directory to scan recursively
    #[arg(default_value = ".")]
    directory: PathBuf,
}

#[derive(Default)]
struct Browser {
    scan: Scan,
    cursor: usize,
    selected: BTreeSet<PathBuf>,
    dialog: Confirmation,
    status: String,
    list: ListState,
    button: Rect,
    tree_view: bool,
    tiles: Vec<Rect>,
    page_start: usize,
}

impl Browser {
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
            self.dialog.request(paths);
            self.cursor = 0;
        }
    }

    fn draw(&mut self, f: &mut Frame, root: &Path, busy: Option<&str>, tick: usize) {
        let [header, directory, tree, status, actions, keys] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(f.area());
        let pending = self.dialog.pending();
        let title = if pending.is_some() {
            "Confirm permanent deletion"
        } else {
            if self.tree_view {
                "Cargo target tree · bar length by file size"
            } else {
                "Cargo target treemap · area by file size"
            }
        };
        f.render_widget(Paragraph::new(title).bold().cyan(), header);
        f.render_widget(Paragraph::new(format!("{:?}", root)).dark_gray(), directory);
        let paths = pending.unwrap_or(&self.scan.targets);
        let [tree, rail] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(tree);
        self.tiles.clear();
        if pending.is_none()
            && !self.tree_view
            && !paths.is_empty()
            && tree.height >= chart::TILE_HEIGHT
            && tree.width >= 16
        {
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
            let page_size = usize::from(tree.width / label_width)
                * usize::from(tree.height / chart::TILE_HEIGHT);
            self.page_start = group_start + (self.cursor - group_start) / page_size * page_size;
            let end = (self.page_start + page_size).min(group_end);
            let page = &paths[self.page_start..end];
            let weights: Vec<_> = page.iter().map(|p| self.scan.bytes[p]).collect();
            chart::tiles(tree, &weights, label_width, &mut self.tiles);
            for (i, ((path, bytes), tile)) in page.iter().zip(weights).zip(&self.tiles).enumerate()
            {
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
                        .bg(size_color(bytes))
                        .block(Block::bordered().border_type(border)),
                    *tile,
                );
            }
        } else {
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
                    .fg(size_color(bytes))
            });
            self.list.select((!paths.is_empty()).then_some(self.cursor));
            f.render_stateful_widget(
                List::new(rows).highlight_style(
                    tuirealm::ratatui::style::Style::default()
                        .bg(Color::DarkGray)
                        .bold(),
                ),
                tree,
                &mut self.list,
            );
            if pending.is_none()
                && let Some(&largest) = self.scan.bytes.values().max()
                && largest > 0
            {
                // List rendering updates its offset to keep the focused entry visible.
                let visible = paths.iter().skip(self.list.offset()).zip(tree.rows());
                for (path, row) in visible {
                    let bytes = self.scan.bytes[path];
                    let width = (u128::from(row.width) * u128::from(bytes))
                        .div_ceil(u128::from(largest)) as u16;
                    let style = tuirealm::ratatui::style::Style::default()
                        .black()
                        .bg(size_color(bytes));
                    f.buffer_mut().set_style(Rect { width, ..row }, style);
                }
            }
        }
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
        let (label, hint) = if let Some(paths) = pending {
            (
                format!(
                    "Permanently remove {} director{} and all contents?",
                    paths.len(),
                    if paths.len() == 1 { "y" } else { "ies" }
                ),
                "Y confirm   Esc / N cancel   Arrows / wheel review",
            )
        } else if let Some(task) = busy {
            let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            (format!("{} {task}", spinner[tick % spinner.len()]), "")
        } else {
            (
                if !self.tree_view
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
                },
                "Arrows / wheel move   Space mark   T tree/map   D delete   Q quit",
            )
        };
        f.render_widget(Paragraph::new(label).yellow(), status);
        self.button = Rect {
            width: actions.width.min(12),
            ..actions
        };
        if busy.is_none() && pending.is_none() && !paths.is_empty() {
            let [_, legend] =
                Layout::horizontal([Constraint::Length(13), Constraint::Min(0)]).areas(actions);
            let bands = SIZE_BANDS
                .iter()
                .map(|&(_, color, label)| Span::from(format!("■ {label}  ")).fg(color));
            f.render_widget(Line::from_iter(bands), legend);
            f.render_widget(
                Paragraph::new(" [D] Delete ").black().on_light_red(),
                self.button,
            );
        }
        f.render_widget(Paragraph::new(hint), keys);
    }
}

fn work(root: PathBuf, paths: Vec<PathBuf>) -> thread::JoinHandle<Result<(Scan, String)>> {
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

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let root = args
        .directory
        .canonicalize()
        .wrap_err_with(|| format!("Cannot open {:?}", args.directory))?;
    spdlog::info!("Scanning {:?}", root);
    let mut terminal = CrosstermTerminalAdapter::new()?;
    terminal.enter_alternate_screen()?;
    terminal.enable_raw_mode()?;
    terminal.enable_mouse_capture()?;
    let mut ui = Browser::default();
    let mut job = Some(work(root.clone(), vec![]));
    let mut task = "Scanning directories";
    let start = Instant::now();
    let mut glide: Option<(usize, Instant)> = None;
    let frame_time = Duration::from_millis(32);
    loop {
        if job.as_ref().is_some_and(|j| j.is_finished())
            && let Some(done) = job.take()
        {
            (ui.scan, ui.status) = done
                .join()
                .map_err(|_| eyre!("Filesystem worker stopped unexpectedly"))??;
            ui.selected.clear();
            ui.cursor = 0;
        }
        if let Some((destination, next_frame)) = glide
            && Instant::now() >= next_frame
        {
            if ui.cursor < destination {
                ui.cursor += 1;
            } else if ui.cursor > destination {
                ui.cursor -= 1;
            }
            glide = (ui.cursor != destination).then(|| (destination, Instant::now() + frame_time));
        }
        terminal.draw(|f| {
            ui.draw(
                f,
                &root,
                job.as_ref().map(|_| task),
                (start.elapsed().as_millis() / 80) as usize,
            )
        })?;
        let timeout = glide.map_or(Duration::from_millis(80), |(_, next_frame)| {
            next_frame.saturating_duration_since(Instant::now())
        });
        if !event::poll(timeout)? {
            continue;
        }
        let key = match event::read()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => {
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
                    KeyCode::Char('q')
                } else {
                    k.code
                }
            }
            Event::Mouse(m)
                if job.is_none()
                    && matches!(
                        m.kind,
                        MouseEventKind::ScrollUp
                            | MouseEventKind::ScrollDown
                            | MouseEventKind::ScrollLeft
                            | MouseEventKind::ScrollRight
                    ) =>
            {
                let forward = matches!(
                    m.kind,
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollRight
                );
                let last = ui
                    .dialog
                    .pending()
                    .unwrap_or(&ui.scan.targets)
                    .len()
                    .saturating_sub(1);
                let (base, next_frame) = glide
                    .filter(|(dest, _)| (*dest > ui.cursor) == forward)
                    .unwrap_or((ui.cursor, Instant::now()));
                let destination = if forward {
                    base.saturating_add(3).min(last)
                } else {
                    base.saturating_sub(3)
                };
                glide = (destination != ui.cursor).then_some((destination, next_frame));
                continue;
            }
            Event::Mouse(m)
                if m.kind == MouseEventKind::Down(MouseButton::Left)
                    && ui.button.contains(Position::new(m.column, m.row)) =>
            {
                KeyCode::Char('d')
            }
            Event::Mouse(m)
                if job.is_none() && m.kind == MouseEventKind::Down(MouseButton::Left) =>
            {
                glide = None;
                if let Some(i) = ui
                    .tiles
                    .iter()
                    .position(|r| r.contains(Position::new(m.column, m.row)))
                {
                    ui.cursor = ui.page_start + i;
                }
                continue;
            }
            _ => continue,
        };
        // Stop on the displayed target before accepting a selection or deletion command.
        glide = None;
        if job.is_some() {
            if key == KeyCode::Char('q') && task == "Scanning directories" {
                break;
            }
            continue;
        }
        let pending = ui.dialog.pending();
        let count = pending.unwrap_or(&ui.scan.targets).len();
        match key {
            KeyCode::Up | KeyCode::Left => ui.cursor = ui.cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Right => {
                ui.cursor = (ui.cursor + 1).min(count.saturating_sub(1))
            }
            KeyCode::Esc | KeyCode::Char('n') if pending.is_some() => {
                ui.dialog.resolve(false);
            }
            KeyCode::Char('y') if pending.is_some() => {
                task = "Deleting confirmed outputs";
                job = Some(work(root.clone(), ui.dialog.resolve(true)));
            }
            _ if pending.is_some() => {}
            KeyCode::Char('t') => ui.tree_view = !ui.tree_view,
            KeyCode::Char('q') => break,
            KeyCode::Char('d') | KeyCode::Delete => ui.request(),
            KeyCode::Char(' ') => {
                if let Some(path) = ui.scan.targets.get(ui.cursor)
                    && !ui.selected.remove(path)
                {
                    ui.selected.insert(path.clone());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use test_that::prelude::*;
    use tuirealm::ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn tree_bars_keep_the_scan_scale_when_scrolled() -> Result<()> {
        let root = Path::new(".");
        let mut ui = Browser {
            tree_view: true,
            ..Browser::default()
        };
        for (name, bytes) in [("max", 80), ("empty", 0), ("quarter", 20), ("half", 40)] {
            let path = root.join(name);
            ui.scan.bytes.insert(path.clone(), bytes);
            ui.scan.targets.push(path);
        }
        let mut terminal = Terminal::new(TestBackend::new(41, 7))?;
        for (cursor, widths) in [(0, [40, 0]), (3, [10, 20])] {
            ui.cursor = cursor;
            terminal.draw(|frame| ui.draw(frame, root, None, 0))?;
            let buf = terminal.backend().buffer();
            for (i, width) in widths.into_iter().enumerate() {
                let row = 2 + i as u16;
                for x in 0..40 {
                    assert_that!(
                        !matches!(buf[(x, row)].bg, Color::Reset | Color::DarkGray),
                        eq(x < width)
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn scrollbar_reaches_the_last_row_in_both_views() -> Result<()> {
        let root = Path::new(".");
        let mut ui = Browser::default();
        for i in 0..32 {
            let path = root.join(i.to_string());
            ui.scan.bytes.insert(path.clone(), 1);
            ui.scan.targets.push(path);
        }
        for tree in [false, true] {
            ui.tree_view = tree;
            let mut terminal = Terminal::new(TestBackend::new(80, 16))?;
            for (cursor, row) in [(0, 2), (31, 12)] {
                ui.cursor = cursor;
                terminal.draw(|f| ui.draw(f, root, None, 0))?;
                assert_that!(terminal.backend().buffer()[(79, row)].symbol(), eq("█"));
            }
        }
        Ok(())
    }

    #[test]
    fn chart_and_tree_share_size_colors_even_for_focused_and_marked_targets() -> Result<()> {
        let mut ui = Browser::default();
        let root = Path::new(".");
        let sizes = [10, 20, 30, 40, 50, 60, 70, 70];
        for (i, bytes) in sizes.into_iter().enumerate() {
            let path = root.join(i.to_string());
            ui.scan.bytes.insert(path.clone(), bytes);
            ui.scan.targets.push(path);
        }
        ui.selected.insert(ui.scan.targets[0].clone());
        let mut terminal = Terminal::new(TestBackend::new(100, 32))?;
        terminal.draw(|f| ui.draw(f, root, None, 0))?;
        let buffer = terminal.backend().buffer();
        let colors: Vec<_> = ui
            .tiles
            .iter()
            .map(|tile| buffer[(tile.x, tile.y)].bg)
            .collect();
        assert_that!(colors.len(), eq(8));
        assert_that!(
            colors[..7]
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            eq(7)
        );
        assert_that!(colors[6], eq(colors[7]));
        for bytes in ui.scan.bytes.values_mut() {
            *bytes *= 1 << 30;
        }
        ui.tree_view = true;
        terminal.draw(|f| ui.draw(f, root, None, 0))?;
        let buffer = terminal.backend().buffer();
        for (i, expected) in colors.into_iter().enumerate() {
            let cell = &buffer[(0, 2 + i as u16)];
            assert_that!(cell.fg == expected || cell.bg == expected, eq(true));
        }
        Ok(())
    }

    #[test]
    fn tiny_outputs_keep_their_names_and_large_outputs_use_readable_units() -> Result<()> {
        let root = Path::new("workspace");
        let mut ui = Browser::default();
        for (name, size) in [("big/target", 1536 * 1024), ("tiny/target", 0)] {
            let path = root.join(name);
            ui.scan.bytes.insert(path.clone(), size);
            ui.scan.targets.push(path);
        }
        let mut terminal = Terminal::new(TestBackend::new(60, 18))?;
        terminal.draw(|frame| ui.draw(frame, root, None, 0))?;
        let buffer = terminal.backend().buffer();
        let tile_text = |tile: Rect| {
            tile.rows()
                .map(|row| {
                    row.positions()
                        .map(|pos| buffer[pos].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_that!(tile_text(ui.tiles[0]).contains("1.5 MiB"), eq(true));
        assert_that!(tile_text(ui.tiles[1]).contains("tiny/target"), eq(true));
        assert_that!(tile_text(ui.tiles[1]).contains("0 B"), eq(true));
        Ok(())
    }

    #[test]
    fn default_view_draws_sized_clickable_blocks() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        for (name, size) in [("small", 100), ("large", 300)] {
            let dir = tmp.path().join(name);
            fs::create_dir(&dir)?;
            fs::write(dir.join(".rustc_info.json"), "")?;
            fs::write(dir.join("artifact"), vec![0; size])?;
        }
        let mut ui = Browser {
            scan: scan(tmp.path())?,
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(85, 24))?;
        terminal.draw(|frame| ui.draw(frame, tmp.path(), None, 0))?;
        let large = ui.tiles[0].area();
        let small = ui.tiles[1].area();
        assert_that!(small, gt(0));
        assert_that!(large, eq(small * 3));
        Ok(())
    }
}
