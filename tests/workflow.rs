use std::{fs, path::PathBuf};
use targetscan::{Confirmation, scan};
use test_that::prelude::*;

#[test]
fn discovers_cargo_outputs_and_prunes_their_contents() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    fs::create_dir_all(root.join("app/target/embedded/target"))?;
    fs::write(root.join("app/Cargo.toml"), "[workspace]")?;
    fs::write(root.join("app/target/embedded/Cargo.toml"), "[workspace]")?;
    fs::create_dir(root.join("custom"))?;
    fs::write(root.join("custom/.rustc_info.json"), "{}")?;
    fs::create_dir(root.join("target"))?;
    let found = scan(&root)?.targets;
    assert_that!(
        found,
        eq(vec![root.join("app/target"), root.join("custom")])
    );
    Ok(())
}

#[test]
fn confirmation_is_cancellable_and_consumed_once() {
    let paths = vec![PathBuf::from("a"), PathBuf::from("b")];
    let mut dialog = Confirmation::default();
    dialog.request(paths.clone());
    assert_that!(dialog.resolve(false).is_empty(), eq(true));
    dialog.request(paths.clone());
    assert_that!(dialog.resolve(true), eq(paths));
    assert_that!(dialog.resolve(true).is_empty(), eq(true));
}
