use anyhow::{anyhow, Result};
use git2::{Commit, ErrorCode, Repository, Signature};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::slice;

pub const README: &str = "README";
pub const LICENSE: &str = "LICENSE";

pub struct TestRepository {
    repository: Repository,
}

impl TestRepository {
    pub fn new<P: AsRef<Path>>(working_dir: P) -> Result<Self> {
        let repository = Repository::init(&working_dir)?;
        Ok(Self { repository })
    }

    pub fn initial_commit(&mut self) -> Result<()> {
        let readme = self.create_readme()?;
        self.stage_path(&readme)?;
        self.commit("Initial commit.")
    }

    pub fn create_readme(&self) -> Result<PathBuf> {
        let path = self.get_working_dir()?.join(README);
        writeln!(File::create(&path)?, "An example README.")?;
        Ok(path)
    }

    pub fn create_license(&self) -> Result<PathBuf> {
        let path = self.get_working_dir()?.join(LICENSE);
        writeln!(File::create(&path)?, "Free as in freedom.")?;
        Ok(path)
    }

    pub fn stage_path<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let working_dir = self.get_working_dir()?;
        let relative_path = path.as_ref().strip_prefix(working_dir)?;

        let mut index = self.repository.index()?;
        index.add_path(relative_path)?;
        index.write()?;

        Ok(())
    }

    pub fn commit(&mut self, message: &str) -> Result<()> {
        let index = self.repository.index()?.write_tree()?;
        let signature = Self::get_signature()?;

        let head_commit = match self.repository.head() {
            Ok(head) => Ok(Some(head.peel_to_commit()?)),
            Err(error) if error.code() == ErrorCode::UnbornBranch => Ok(None),
            Err(error) => Err(error),
        }?;

        let head_commit_ref = head_commit.as_ref();

        let parents = match head_commit_ref {
            Some(ref commit) => slice::from_ref(commit),
            None => &[] as &[&Commit],
        };

        self.repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &self.repository.find_tree(index)?,
            parents,
        )?;

        Ok(())
    }

    fn get_working_dir(&self) -> Result<&Path> {
        self.repository
            .workdir()
            .ok_or_else(|| anyhow!("Could not find the working directory."))
    }

    fn get_signature<'a>() -> Result<Signature<'a>> {
        Ok(Signature::now("me", "me@example.com")?)
    }
}
