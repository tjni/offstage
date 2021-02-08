use anyhow::{anyhow, Context, Result};
use git2::{Delta, ErrorCode, Oid, Repository, Signature, StashFlags};
use itertools::Itertools;
use std::collections::HashSet;
use std::fs;
use std::hash::Hash;
use std::io::ErrorKind::NotFound;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

pub struct GitRepository {
    repository: Repository,
}

impl GitRepository {

    pub fn open() -> Result<Self> {
        let repository = Repository::open_from_env()
            .with_context(|| "Encountered an error when opening the Git repository.")?;

        Ok(Self { repository, })
    }

    pub fn save_snapshot(&mut self) -> Result<()> {
        let partially_staged_files = self.get_partially_staged_files()?;

        let deleted_files = self.get_deleted_files()?;

        println!("Partially staged {:?}", partially_staged_files);
        // println!("Deleted {:?}", deleted_files);

        //let stash = self.save_snapshot_stash()?;

        // Because `git stash` restores the HEAD commit, it brings back uncommitted
        // deleted files. We need to clear them before creating our snapshot.
        Self::delete_files(&deleted_files)?;

        Ok(())
    }

    fn get_partially_staged_files(&self) -> Result<HashSet<PathBuf>> {
        let head_tree = self.repository.head()?.peel_to_tree()?;

        let staged_files = HashSet::from_iter(self.repository
            .diff_tree_to_index(Some(&head_tree), None, None)?
            .deltas()
            .flat_map(|delta| vec![
                delta.old_file().path(),
                delta.new_file().path(),
            ])
            .filter_map(std::convert::identity)
            .map(Path::to_path_buf));

        let unstaged_files = HashSet::from_iter(self.repository
            .diff_index_to_workdir(None, None)?
            .deltas()
            .flat_map(|delta| vec![
                delta.old_file().path(),
                delta.new_file().path(),
            ])
            .filter_map(std::convert::identity)
            .map(Path::to_path_buf));

        fn intersect<P: Eq + Hash>(one: HashSet<P>, two: &HashSet<P>) -> HashSet<P> {
            one.into_iter().filter(|p| two.contains(p)).collect()
        }

        Ok(intersect(staged_files, &unstaged_files))
    }

    fn get_deleted_files(&self) -> Result<Vec<PathBuf>> {
        let deleted_files = self.repository
            .diff_index_to_workdir(None, None)?
            .deltas()
            .filter(|delta| delta.status() == Delta::Deleted)
            .filter_map(|delta| delta.old_file().path())
            .map(Path::to_path_buf)
            .collect_vec();

        Ok(deleted_files)
    }

    fn save_snapshot_stash(&mut self) -> Result<Option<Oid>> {
        // Save state when in the middle of a merge prior to stashing changes in
        // the working directory so that we can restore it afterward.
        let merge_status = self.save_merge_status()?;

        let dummy_signature = Signature::now("Offstage Dummy User", "dummy@example.com")
            .with_context(|| "Encountered an error when creating dummy authorship information.")?;

        let stash_result = self.repository.stash_save(
            &dummy_signature,
            "offstage backup",
            Some(StashFlags::INCLUDE_UNTRACKED | StashFlags::KEEP_INDEX),
        );

        self.restore_merge_status(&merge_status)?;

        match stash_result {
            Ok(stash_id) => Ok(Some(stash_id)),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(anyhow!(error).context(
                "Encountered an error when stashing a backup of the working directory.")),
        }
    }

    fn save_merge_status(&self) -> Result<MergeStatus> {
        let merge_head_path = &self.repository.path().join("MERGE_HEAD");
        let merge_head = Self::read_file_to_string(merge_head_path)
            .with_context(|| format!("Encountered an error when saving {}.", merge_head_path.display()))?;

        let merge_mode_path = &self.repository.path().join("MERGE_MODE");
        let merge_mode = Self::read_file_to_string(merge_mode_path)
            .with_context(|| format!("Encountered an error when saving {}.", merge_mode_path.display()))?;

        let merge_msg_path = &self.repository.path().join("MERGE_MSG");
        let merge_msg = Self::read_file_to_string(merge_msg_path)
            .with_context(|| format!("Encountered an error when saving {}.", merge_msg_path.display()))?;

        Ok(MergeStatus { merge_head, merge_mode, merge_msg, })
    }

    fn restore_merge_status(&self, merge_status: &MergeStatus) -> Result<()> {
        // Tries to restore all files before returning the first error if one exists.

        let restore_merge_head_result = merge_status.merge_head.as_ref().map_or(Ok(()), |merge_head| {
            let merge_head_path = &self.repository.path().join("MERGE_HEAD");
            fs::write(merge_head_path, merge_head)
                .with_context(|| format!("Encountered an error when restoring {}.", merge_head_path.display()))
        });

        let restore_merge_mode_result = merge_status.merge_mode.as_ref().map_or(Ok(()), |merge_mode| {
            let merge_mode_path = &self.repository.path().join("MERGE_MODE");
            fs::write(merge_mode_path, merge_mode)
                .with_context(|| format!("Encountered an error when restoring {}.", &merge_mode_path.display()))
        });

        let restore_merge_msg_result = merge_status.merge_msg.as_ref().map_or(Ok(()), |merge_msg| {
            let merge_msg_path = &self.repository.path().join("MERGE_MSG");
            fs::write(merge_msg_path, merge_msg)
                .with_context(|| format!("Encountered an error when restoring {}.", merge_msg_path.display()))
        });

        restore_merge_head_result?;
        restore_merge_mode_result?;
        restore_merge_msg_result?;

        Ok(())
    }

    fn read_file_to_string<P: AsRef<Path>>(file: P) -> Result<Option<String>> {
        match fs::read_to_string(file) {
            Ok(contents) => Ok(Some(contents)),
            Err(error) if error.kind() == NotFound => Ok(None),
            Err(error) => Err(anyhow!(error)),
        }
    }

    fn delete_files<P: AsRef<Path>>(files: &Vec<P>) -> Result<()> {
        for file in files.iter() {
            fs::remove_file(file)
                .with_context(|| format!("Encountered error when deleting {}.", file.as_ref().display()))?;
        }

        Ok(())
    }
}

#[derive(Debug)]
struct MergeStatus {
    merge_head: Option<String>,
    merge_mode: Option<String>,
    merge_msg: Option<String>,
}