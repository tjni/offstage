use anyhow::Result;
use duct::cmd;
use repository::TestRepository;
use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;

mod repository;

const OUTPUTS_DIR: &str = "tests/outputs";
const BINARY_NAME: &str = env!("CARGO_BIN_EXE_offstage");

static INIT: Once = Once::new();

fn initialize(test_name: &str) -> Result<PathBuf> {
    let outputs_dir = Path::new(OUTPUTS_DIR);

    INIT.call_once(|| {
        if !outputs_dir.is_dir() {
            fs::create_dir(outputs_dir).expect("The output directory couldn't be created.");
        }
    });

    let test_output_dir = outputs_dir.join(test_name);
    if test_output_dir.is_dir() {
        fs::remove_dir_all(&test_output_dir)?;
    }

    fs::create_dir(&test_output_dir)?;
    Ok(test_output_dir)
}

#[test]
fn empty_stage_in_empty_repository_skips_command() -> Result<()> {
    // Given
    let working_dir = initialize("empty_stage_in_empty_repository_skips_command")?;
    TestRepository::new(&working_dir)?;

    // When
    let marker = "marker";
    let stdout = cmd!(BINARY_NAME, "echo", marker).dir(&working_dir).read()?;

    // Then
    assert!(
        !stdout.contains(marker),
        "Output \"{}\" should not contain \"{}\".",
        stdout,
        marker
    );

    Ok(())
}

#[test]
fn empty_stage_in_repository_skips_command() -> Result<()> {
    // Given
    let working_dir = initialize("empty_stage_in_repository_skips_command")?;
    let mut repository = TestRepository::new(&working_dir)?;

    repository.initial_commit()?;

    // When
    let marker = "marker";
    let stdout = cmd!(BINARY_NAME, "echo", marker).dir(&working_dir).read()?;

    // Then
    assert!(
        !stdout.contains(marker),
        "Output \"{}\" should not contain \"{}\".",
        stdout,
        marker
    );

    Ok(())
}

#[test]
fn untracked_file_remains_after_command_succeeds() -> Result<()> {
    // Given
    let working_dir = initialize("untracked_file_remains_after_command_succeeds")?;

    let mut repository = TestRepository::new(&working_dir)?;

    repository.initial_commit()?;

    let readme_relative_path = "README";
    let mut readme_file = OpenOptions::new()
        .append(true)
        .open(&working_dir.join(readme_relative_path))?;

    writeln!(readme_file, "A new line!")?;

    repository.stage_path(readme_relative_path)?;

    let untracked_file_relative_path = "untracked.txt";
    let untracked_file = &working_dir.join(untracked_file_relative_path);
    writeln!(
        File::create(untracked_file)?,
        "This text file is untracked."
    )?;

    // When
    let marker = "marker";
    let stdout = cmd!(BINARY_NAME, "echo", marker).dir(&working_dir).read()?;

    // Then
    assert!(
        stdout.contains(marker),
        "Output \"{}\" should contain \"{}\".",
        stdout,
        marker
    );

    assert!(
        !stdout.contains(untracked_file_relative_path),
        "Output \"{}\" should not contain \"{}\".",
        stdout,
        untracked_file_relative_path
    );

    assert!(untracked_file.is_file(), "The untracked file should still exist.");

    Ok(())
}
