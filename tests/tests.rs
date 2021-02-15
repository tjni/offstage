use anyhow::Result;
use assert_cmd::{assert::Assert, Command};
use predicates::prelude::*;
use repository::TestRepository;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

mod repository;

const OUTPUTS_DIR: &str = "tests/outputs";
const BINARY_NAME: &str = "offstage";

static INIT: Once = Once::new();

fn initialize(test_name: &str) -> Result<PathBuf> {
    let outputs_dir = Path::new(OUTPUTS_DIR);

    INIT.call_once(|| {
        if !outputs_dir.is_dir() {
            fs::create_dir(outputs_dir)
                .expect("The output directory couldn't be created.");
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

    let mut command = TestCommand::new(&working_dir)?;

    // When
    let assert = command.run(vec!["echo", "marker"]);

    // Then
    assert
        .success()
        .stdout(predicate::str::contains("marker").not());

    Ok(())
}

#[test]
fn empty_stage_in_repository_skips_command() -> Result<()> {
    // Given
    let working_dir = initialize("empty_stage_in_repository_skips_command")?;
    let mut repository = TestRepository::new(&working_dir)?;

    repository.initial_commit()?;

    let mut command = TestCommand::new(&working_dir)?;

    // When
    let assert = command.run(vec!["echo", "marker"]);

    // Then
    assert
        .success()
        .stdout(predicate::str::contains("marker").not());

    Ok(())
}

struct TestCommand {
    command: Command,
}

impl TestCommand {
    fn new<P: AsRef<Path>>(current_dir: P) -> Result<TestCommand> {
        let mut command = Command::cargo_bin(BINARY_NAME)?;
        command.current_dir(current_dir);

        Ok(TestCommand { command })
    }

    fn run<I, S>(&mut self, args: I) -> Assert
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.command.args(args).assert()
    }
}
