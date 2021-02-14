use super::git::{GitRepository, Snapshot};
use anyhow::Result;
use duct::cmd;
use itertools::Itertools;
use std::path::Path;

/// Runs the core logic to back up the working directory, apply a command to the
/// staged files, and handle errors.
pub fn run<P: AsRef<Path>>(shell: P, command: &Vec<String>) -> Result<()> {
    if let Some(mut workflow) = Workflow::prepare()? {
        let result = workflow.run(shell, command);

        // TODO: We need to aggregate these errors and show all of them.

        // TODO: We need to show a message when a commit was prevented because it
        // would be an empty commit.

        if result.is_err() {
            workflow.restore()?;
        }

        workflow.cleanup()?;

        return result;
    }

    Ok(())
}

struct Workflow {
    repository: GitRepository,
    snapshot: Snapshot,
}

impl Workflow {
    fn prepare() -> Result<Option<Self>> {
        let mut repository = GitRepository::open()?;

        let staged_files = repository.get_staged_files()?;

        if staged_files.is_empty() {
            return Ok(None);
        }

        let snapshot = repository.save_snapshot(staged_files)?;

        Ok(Some(Self {
            repository,
            snapshot,
        }))
    }

    fn run<P: AsRef<Path>>(&mut self, shell: P, command: &Vec<String>) -> Result<()> {
        let staged_files_iter = self
            .snapshot
            .staged_files
            .iter()
            .filter_map(|path| path.to_str());

        let command = command
            .iter()
            .map(String::as_str)
            .chain(staged_files_iter)
            .join(" ");

        cmd!(shell.as_ref(), "-c", command).run()?;

        self.repository.apply_modifications(&self.snapshot)
    }

    fn restore(&mut self) -> Result<()> {
        self.repository.restore_snapshot(&self.snapshot)
    }

    fn cleanup(mut self) -> Result<()> {
        self.repository.clean_snapshot(self.snapshot)
    }
}
