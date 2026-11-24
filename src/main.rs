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
        widgets::{List, ListItem, ListState, Paragraph},
    },
    terminal::{CrosstermTerminalAdapter, TerminalAdapter},
};

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
            "Cargo target explorer"
        };
        f.render_widget(Paragraph::new(title).bold().cyan(), header);
        f.render_widget(Paragraph::new(format!("{:?}", root)).dark_gray(), directory);
        let paths = pending.unwrap_or(&self.scan.targets);
        let rows = paths.iter().enumerate().map(|(i, path)| {
            let branch = if i + 1 == paths.len() {
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
            ListItem::new(format!("{branch} {mark} {name:?}"))
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
        let (label, hint) = if let Some(paths) = pending {
            (
                format!(
                    "Permanently remove {} director{} and all contents?",
                    paths.len(),
                    if paths.len() == 1 { "y" } else { "ies" }
                ),
                "Y confirm   Esc / N cancel   ↑↓ review",
            )
        } else if let Some(task) = busy {
            let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            (format!("{} {task}", spinner[tick % spinner.len()]), "")
        } else {
            (
                self.status.clone(),
                "↑↓ move   Space mark   D delete   Q quit",
            )
        };
        f.render_widget(Paragraph::new(label).yellow(), status);
        self.button = Rect {
            width: actions.width.min(12),
            ..actions
        };
        if busy.is_none() && pending.is_none() && !paths.is_empty() {
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
        terminal.draw(|f| {
            ui.draw(
                f,
                &root,
                job.as_ref().map(|_| task),
                (start.elapsed().as_millis() / 80) as usize,
            )
        })?;
        if !event::poll(Duration::from_millis(80))? {
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
                if m.kind == MouseEventKind::Down(MouseButton::Left)
                    && ui.button.contains(Position::new(m.column, m.row)) =>
            {
                KeyCode::Char('d')
            }
            _ => continue,
        };
        if job.is_some() {
            if key == KeyCode::Char('q') && task == "Scanning directories" {
                break;
            }
            continue;
        }
        let pending = ui.dialog.pending();
        let count = pending.unwrap_or(&ui.scan.targets).len();
        match key {
            KeyCode::Up => ui.cursor = ui.cursor.saturating_sub(1),
            KeyCode::Down => ui.cursor = (ui.cursor + 1).min(count.saturating_sub(1)),
            KeyCode::Esc | KeyCode::Char('n') if pending.is_some() => {
                ui.dialog.resolve(false);
            }
            KeyCode::Char('y') if pending.is_some() => {
                task = "Deleting confirmed outputs";
                job = Some(work(root.clone(), ui.dialog.resolve(true)));
            }
            _ if pending.is_some() => {}
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
