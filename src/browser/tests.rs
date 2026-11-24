use super::Browser;
use color_eyre::Result;
use std::fs;
use std::path::{Path, PathBuf};
use targetscan::{Scan, scan};
use test_that::prelude::*;
use tuirealm::ratatui::{Terminal, backend::TestBackend};
use tuirealm::ratatui::{layout::Rect, style::Color};

#[test]
fn tree_orders_by_descending_bytes_then_path_and_keeps_focus() {
    let mut ui = Browser::default();
    let fixture = |a_size| {
        let bytes = [
            (PathBuf::from("a"), a_size),
            ("b".into(), 25),
            ("c".into(), 25),
        ]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
        Scan {
            targets: bytes.keys().cloned().collect(),
            bytes,
            skipped: 0,
        }
    };
    ui.refresh(fixture(0), String::new());
    let paths = |names: [&str; 3]| names.map(PathBuf::from).to_vec();
    assert_that!(ui.scan.targets, eq(paths(["b", "c", "a"])));
    assert_that!(ui.cursor, eq(0));
    ui.refresh(fixture(80), String::new());
    assert_that!(ui.scan.targets, eq(paths(["a", "b", "c"])));
    assert_that!(ui.cursor, eq(1));
    ui.refresh(fixture(0), String::new());
    ui.toggle_view();
    assert_that!(ui.scan.targets, eq(paths(["a", "b", "c"])));
    assert_that!(ui.cursor, eq(1));
}

#[test]
fn background_delete_preserves_browsing_and_marks_across_refresh() -> Result<()> {
    let mut ui = Browser::default();
    let a = PathBuf::from("a");
    let b = PathBuf::from("b");
    let c = PathBuf::from("c");
    ui.scan.targets = vec![a.clone(), b.clone(), c.clone()];
    ui.scan.bytes = [(a.clone(), 1), (b.clone(), 2), (c.clone(), 3)].into();
    ui.cursor = 2;
    ui.selected.insert(c.clone());
    let mut terminal = Terminal::new(TestBackend::new(100, 24))?;
    terminal.draw(|f| ui.draw(f, Path::new("."), Some("Deleting confirmed outputs"), 0))?;
    let hint: String = (0..100)
        .map(|x| terminal.backend().buffer()[(x, 23)].symbol())
        .collect();
    assert_that!(hint.contains("Space mark"), eq(true));
    let refreshed = Scan {
        targets: vec![b.clone(), c.clone()],
        bytes: [(b, 2), (c.clone(), 3)].into(),
        skipped: 0,
    };
    ui.refresh(refreshed, "Deleted".into());
    assert_that!(ui.scan.targets[ui.cursor], eq(c.clone()));
    assert_that!(ui.selected.contains(&c), eq(true));
    Ok(())
}

#[test]
fn tree_bars_keep_the_scan_scale_when_scrolled() -> Result<()> {
    let root = Path::new(".");
    let mut ui = Browser::default();
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
    for treemap in [false, true] {
        ui.treemap_view = treemap;
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
    let mut ui = Browser {
        treemap_view: true,
        ..Browser::default()
    };
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
    ui.treemap_view = false;
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
    let mut ui = Browser {
        treemap_view: true,
        ..Browser::default()
    };
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
fn toggling_from_the_default_tree_draws_sized_clickable_blocks() -> Result<()> {
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
    assert_that!(ui.treemap_view, eq(false));
    ui.toggle_view();
    let mut terminal = Terminal::new(TestBackend::new(85, 24))?;
    terminal.draw(|frame| ui.draw(frame, tmp.path(), None, 0))?;
    let large = ui.tiles[0].area();
    let small = ui.tiles[1].area();
    assert_that!(small, gt(0));
    assert_that!(large, eq(small * 3));
    Ok(())
}

#[test]
fn visible_footer_labels_dispatch_their_shortcuts() -> Result<()> {
    use crossterm::event::KeyCode;
    use tuirealm::ratatui::layout::Position;
    let mut ui = Browser::default();
    let path = PathBuf::from("target");
    ui.scan.targets.push(path.clone());
    ui.selected.insert(path.clone());
    ui.scan.bytes.insert(path, 1);
    let mut terminal = Terminal::new(TestBackend::new(100, 16))?;
    for review in [false, true] {
        if review {
            ui.request();
        }
        terminal.draw(|f| ui.draw(f, Path::new("."), None, 0))?;
        let buf = terminal.backend().buffer();
        let row: String = (0..100).map(|x| buf[(x, 15)].symbol()).collect();
        let labels = if review {
            vec![("confirm", KeyCode::Char('y')), ("cancel", KeyCode::Esc)]
        } else {
            vec![
                ("mark", KeyCode::Char(' ')),
                ("select all", KeyCode::Char('a')),
                ("clear marks", KeyCode::Esc),
                ("tree/map", KeyCode::Char('t')),
                ("delete", KeyCode::Char('d')),
                ("quit", KeyCode::Char('q')),
            ]
        };
        for (label, key) in labels {
            let start = row
                .find(label)
                .ok_or_else(|| color_eyre::eyre::eyre!("missing {label} in {row}"))?;
            let col = row[..start].chars().count() as u16;
            assert_that!(ui.footer_key(Position::new(col, 15)), eq(Some(key)));
        }
        assert_that!(ui.footer_key(Position::new(99, 15)), eq(None));
    }
    Ok(())
}
