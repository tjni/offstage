use anyhow::Result;
use assert_cmd::{assert::Assert, Command};
use git2::Repository;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

const BINARY_NAME: &str = "offstage";

#[test]
fn test_empty_stage_in_empty_repository_skips_command() -> Result<()> {
    // Given
    let tmp_dir = TempDir::new()?;

    Repository::init(&tmp_dir)?;
    
    let mut command = TestCommand::new(&tmp_dir)?;

    // When
    let assert = command.run(vec!["echo", "marker"]);

    // Then
    assert.success().stdout(predicate::str::contains("marker").not());

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
