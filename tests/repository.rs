use anyhow::{anyhow, Result};
use git2::{Oid, Repository, Signature};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct TestRepository {
    repository: Repository,
}

impl TestRepository {
    pub fn new<P: AsRef<Path>>(working_dir: P) -> Result<Self> {
        let repository = Repository::init(&working_dir)?;
        Ok(Self { repository })
    }

    pub fn initial_commit(&mut self) -> Result<()> {
        let workdir = self.repository
            .workdir()
            .ok_or_else(|| anyhow!("Could not find the working directory."))?;

        let relative_path = Path::new("README");
        writeln!(
            File::create(&workdir.join(relative_path))?,
            "An example README."
        )?;

        let index = self.add_to_index(relative_path)?;
        let signature = Self::get_signature()?;

        self.repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit.",
            &self.repository.find_tree(index)?,
            &vec![],
        )?;

        Ok(())
    }

    fn add_to_index(&mut self, relative_path: &Path) -> Result<Oid> {
        let mut index = self.repository.index()?;
        index.add_path(relative_path)?;
        index.write()?;

        Ok(index.write_tree()?)
    }

    fn get_signature<'a>() -> Result<Signature<'a>> {
        Ok(Signature::now("me", "me@example.com")?)
    }
}