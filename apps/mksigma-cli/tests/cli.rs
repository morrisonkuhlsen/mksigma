//! `mksigma-cli run` as a person runs it: the files it writes when one of them can't be.

use std::path::PathBuf;
use std::process::Command;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mksigma-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mksigma-cli")).arg("run").args(args).output().unwrap()
}

#[test]
fn a_failed_export_still_writes_the_data_and_the_other_files() {
    let dir = folder("exports");
    let out = dir.join("result.sav");
    let html = dir.join("report.html");
    let bad = dir.join("missing/board.tex");
    let result = run(&["--out", out.to_str().unwrap(), "--tex", bad.to_str().unwrap(), "--html", html.to_str().unwrap(), "COMPUTE twice = age * 2."]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("board.tex"));
    assert!(std::fs::metadata(&out).unwrap().len() > 0, "the data was written");
    assert!(std::fs::metadata(&html).unwrap().len() > 0, "and the export after the failed one");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_failed_save_leaves_the_file_there_untouched() {
    let dir = folder("save");
    let data = dir.join("data.sav");
    assert!(run(&["--out", data.to_str().unwrap(), "DESCRIPTIVES age."]).status.success());
    let before = std::fs::read(&data).unwrap();
    // A folder in the way of the temporary file's rename: nothing replaces the data.
    let blocked = dir.join("blocked.sav");
    std::fs::create_dir(&blocked).unwrap();
    std::fs::write(blocked.join("inside"), b"x").unwrap();
    let result = run(&["--in", data.to_str().unwrap(), "--out", blocked.to_str().unwrap(), "COMPUTE twice = age * 2."]);
    assert!(!result.status.success());
    assert_eq!(std::fs::read(&data).unwrap(), before);
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(left.iter().all(|n| !n.ends_with(".saving")), "{left:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}
