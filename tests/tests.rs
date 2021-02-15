use anyhow::Result;
use duct::cmd;
use repository::{TestRepository, LICENSE, README};
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;

mod repository;

const OUTPUTS_DIR: &str = "tests/outputs";
const BINARY_NAME: &str = env!("CARGO_BIN_EXE_offstage");

static INIT: Once = Once::new();

fn initialize(test_name: &str) -> Result<PathBuf> {
    let current_dir = env::current_dir().expect("No current directory found.");
    let outputs_dir = current_dir.join(OUTPUTS_DIR);

    INIT.call_once(|| {
        if !outputs_dir.is_dir() {
            fs::create_dir(&outputs_dir).expect("The output directory couldn't be created.");
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

    let readme = working_dir.join(README);
    append_line(&readme, "A new line.")?;
    println!("Readme path: {:?}", &readme);
    repository.stage_path(&readme)?;
    println!("4");

    let license = repository.create_license()?;

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
        !stdout.contains(LICENSE),
        "Output \"{}\" should not contain untracked file \"{}\".",
        stdout,
        LICENSE
    );

    assert!(
        license.is_file(),
        "The untracked file {} should still exist.",
        LICENSE
    );

    Ok(())
}

#[test]
fn unstaged_file_remains_after_command_succeeds() -> Result<()> {
    // Given
    let working_dir = initialize("unstaged_file_remains_after_command_succeeds")?;

    let mut repository = TestRepository::new(&working_dir)?;

    repository.initial_commit()?;

    let license = repository.create_license()?;
    repository.stage_path(&license)?;
    repository.commit("Add a license file.")?;

    let readme = working_dir.join(README);
    append_line(&readme, "A new line.")?;
    repository.stage_path(&readme)?;

    append_line(&license, "A new line.")?;

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
        !stdout.contains(LICENSE),
        "Output \"{}\" should not contain unstaged file \"{}\".",
        stdout,
        LICENSE
    );

    let license_contents = fs::read_to_string(&license)?;

    assert!(
        license_contents.contains("A new line."),
        "The unstaged file {} should still contain its changes.",
        LICENSE
    );

    Ok(())
}

fn append_line<P: AsRef<Path>>(path: P, line: &str) -> Result<()> {
    let mut file = OpenOptions::new().append(true).open(path.as_ref())?;

    writeln!(file, "\n{}", line)?;

    Ok(())
}
