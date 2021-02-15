use anyhow::{anyhow, Result};
use assert_cmd::{assert::Assert, Command};
use git2::{Repository, Signature};
use predicates::prelude::*;
use std::fs::File;
use std::io::Write;
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
    assert
        .success()
        .stdout(predicate::str::contains("marker").not());

    Ok(())
}

#[test]
fn test_empty_stage_in_repository_skips_command() -> Result<()> {
    // Given
    let tmp_dir = TempDir::new()?;

    let repository = Repository::init(&tmp_dir)?;

    create_initial_commit(&repository)?;

    let mut command = TestCommand::new(&tmp_dir)?;

    // When
    let assert = command.run(vec!["echo", "marker"]);

    // Then
    assert
        .success()
        .stdout(predicate::str::contains("marker").not());

    Ok(())
}

fn create_initial_commit(repository: &Repository) -> Result<()> {
    let workdir = repository
        .workdir()
        .ok_or_else(|| anyhow!("Could not find the working directory."))?;

    let relative_path = Path::new("README");
    writeln!(
        File::create(&workdir.join(relative_path))?,
        "An example README."
    )?;

    let mut index = repository.index()?;
    index.add_path(relative_path)?;
    index.write()?;

    let index_oid = index.write_tree()?;

    let signature = Signature::now("me", "me@example.com")?;

    repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit.",
        &repository.find_tree(index_oid)?,
        &vec![],
    )?;

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
