use super::Browser;
use crate::worker::work;
use color_eyre::eyre::{Result, eyre};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    time::{Duration, Instant},
};
use tuirealm::{
    ratatui::layout::Position,
    terminal::{CrosstermTerminalAdapter, TerminalAdapter},
};

pub(crate) fn run(root: PathBuf) -> Result<()> {
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
            let (scan, status) = done
                .join()
                .map_err(|_| eyre!("Filesystem worker stopped unexpectedly"))??;
            ui.refresh(scan, status);
            glide = None;
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
                if matches!(
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
            Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                let position = Position::new(m.column, m.row);
                if let Some(key) = ui.footer_key(position) {
                    key
                } else {
                    glide = None;
                    if let Some(i) = ui.tiles.iter().position(|r| r.contains(position)) {
                        ui.cursor = ui.page_start + i;
                    }
                    continue;
                }
            }
            _ => continue,
        };
        // Stop on the displayed target before accepting a selection or deletion command.
        glide = None;
        let pending = ui.dialog.pending();
        let count = pending.unwrap_or(&ui.scan.targets).len();
        match key {
            KeyCode::Up | KeyCode::Left => ui.cursor = ui.cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Right => {
                ui.cursor = (ui.cursor + 1).min(count.saturating_sub(1))
            }
            KeyCode::Esc | KeyCode::Char('n') if pending.is_some() => {
                ui.dialog.resolve(false);
                ui.cursor = ui.browse_cursor;
            }
            KeyCode::Char('y') if pending.is_some() => {
                task = "Deleting confirmed outputs";
                let paths = ui.dialog.resolve(true);
                let removing: BTreeSet<_> = paths.iter().collect();
                let next_cursor = ui.scan.targets[..ui.browse_cursor]
                    .iter()
                    .filter(|p| !removing.contains(*p))
                    .count();
                ui.scan.targets.retain(|path| !removing.contains(path));
                for path in &paths {
                    ui.selected.remove(path);
                    ui.scan.bytes.remove(path);
                }
                ui.cursor = next_cursor.min(ui.scan.targets.len().saturating_sub(1));
                job = Some(work(root.clone(), paths));
            }
            _ if pending.is_some() => {}
            KeyCode::Char('t') => ui.toggle_view(),
            KeyCode::Char('q') => break,
            KeyCode::Char('d') | KeyCode::Delete if job.is_none() => ui.request(),
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
    drop(terminal);
    // Leaving the TUI must not interrupt a confirmed recursive deletion mid-directory.
    if task == "Deleting confirmed outputs"
        && let Some(active) = job
    {
        eprintln!("Finishing confirmed deletion…");
        let (_, status) = active
            .join()
            .map_err(|_| eyre!("Filesystem worker stopped unexpectedly"))??;
        eprintln!("{status}");
    }
    Ok(())
}
