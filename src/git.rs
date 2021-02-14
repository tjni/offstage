use anyhow::{anyhow, Context, Result};
use git2::{
    build::CheckoutBuilder, ApplyLocation, Delta, Diff, DiffFormat, DiffOptions, ErrorCode,
    IndexAddOption, Oid, Repository, ResetType, Signature, StashApplyOptions, Time,
};
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::hash::Hash;
use std::io::ErrorKind::NotFound;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

/// An abstraction over a Git repository providing complex behavior needed for
/// applying changes to staged files safely.
pub struct GitRepository {
    repository: Repository,
}

impl GitRepository {
    /// Attempts to open an already-existing repository.
    ///
    /// If the $GIT_DIR environment variable is set, this uses it to locate the
    /// Git repository. Otherwise, this searches up the directory tree from the
    /// current directory to find the repository.
    pub fn open() -> Result<Self> {
        // When strict hash verification is disabled, it means libgit2 will not
        // compute the "object id" of Git objects (which is a SHA-1 hash) after
        // reading them to verify they match the object ids being used to look
        // them up. This improves performance, and I don't have in front of me
        // a concrete example where this is necessary to prevent data loss. If
        // one becomes obvious, then we should make this configurable.
        //
        git2::opts::strict_hash_verification(false);

        let repository = Repository::open_from_env()
            .with_context(|| "Encountered an error when opening the Git repository.")?;

        Ok(Self { repository })
    }

    pub fn save_snapshot(&mut self, staged_files: Vec<PathBuf>) -> Result<Snapshot> {
        let inner = || -> Result<Snapshot> {
            let deleted_files = self.get_deleted_files()?;
            let unstaged_diff = self.save_unstaged_diff()?;
            let backup_stash = self.save_snapshot_stash()?;

            // Because `git stash` restores the HEAD commit, it brings back uncommitted
            // deleted files. We need to clear them before creating our snapshot.
            GitRepository::delete_files(&deleted_files)?;

            self.hide_partially_staged_changes()?;

            Ok(Snapshot {
                backup_stash,
                staged_files,
                unstaged_diff,
            })
        };

        inner().with_context(|| "Encountered an error when saving a snapshot.")
    }

    pub fn apply_modifications(&mut self, snapshot: &Snapshot) -> Result<()> {
        self.stage_modifications(snapshot)?;

        if self.get_staged_files()?.is_empty() {
            return Err(anyhow!("Prevented an empty git commit."));
        }

        if let Some(raw_diff) = &snapshot.unstaged_diff {
            let unstaged_diff = Diff::from_buffer(raw_diff)?;
            self.merge_modifications(unstaged_diff)?;
        }

        Ok(())
    }

    pub fn restore_snapshot(&mut self, snapshot: &Snapshot) -> Result<()> {
        let mut inner = || -> Result<()> {
            self.hard_reset()?;

            if let Some(backup_stash) = &snapshot.backup_stash {
                self.apply_stash(&backup_stash.stash_id)?;
                self.restore_merge_status(&backup_stash.merge_status)?;
            }

            Ok(())
        };

        inner().with_context(|| "Encountered an error when restoring snapshot after another error.")
    }

    pub fn clean_snapshot(&mut self, snapshot: Snapshot) -> Result<()> {
        let inner = || -> Result<()> {
            if let Some(backup_stash) = snapshot.backup_stash {
                let stash_index = self
                    .get_stash_index_from_id(&backup_stash.stash_id)?
                    .ok_or_else(|| {
                        anyhow!(
                            "Could not find a backup stash with id {}.",
                            &backup_stash.stash_id
                        )
                    })?;

                self.repository.stash_drop(stash_index)?;
            }

            Ok(())
        };

        inner().with_context(|| {
            "Encountered an error when cleaning snapshot. You might find a stash entry \
             in the stash list."
        })
    }

    fn stage_modifications(&mut self, snapshot: &Snapshot) -> Result<()> {
        let mut index = self.repository.index()?;
        index.add_all(
            &snapshot.staged_files,
            IndexAddOption::DEFAULT | IndexAddOption::DISABLE_PATHSPEC_MATCH,
            None,
        )?;
        index.write()?;
        Ok(())
    }

    fn merge_modifications(&self, unstaged_diff: Diff) -> Result<()> {
        self.repository
            .apply(&unstaged_diff, ApplyLocation::WorkDir, None)
            .with_context(|| "Unstaged changes could not be restored due to a merge conflict.")
    }

    fn hard_reset(&self) -> Result<()> {
        let head = self.repository.head()?.peel_to_commit()?;

        self.repository
            .reset(head.as_object(), ResetType::Hard, None)
            .map_err(|error| anyhow!(error))
    }

    fn get_stash_index_from_id(&mut self, stash_id: &Oid) -> Result<Option<usize>> {
        // It would be much better if libgit2 accepted a stash Oid
        // instead of an index from the stash list.
        let ref_stash_index = RefCell::new(None);

        self.repository.stash_foreach(|index, _, oid| {
            if oid == stash_id {
                *ref_stash_index.borrow_mut() = Some(index);
                false
            } else {
                true
            }
        })?;

        // Copy the data out of the RefCell.
        let stash_index = match *ref_stash_index.borrow() {
            Some(index) => Some(index),
            None => None,
        };

        Ok(stash_index)
    }

    fn apply_stash(&mut self, stash_id: &Oid) -> Result<()> {
        let stash_index = self
            .get_stash_index_from_id(stash_id)?
            .ok_or_else(|| anyhow!("Could not find a backup stash with id {}.", stash_id))?;

        self.repository.stash_apply(
            stash_index,
            Some(StashApplyOptions::default().reinstantiate_index()),
        )?;

        Ok(())
    }

    fn save_unstaged_diff(&self) -> Result<Option<Vec<u8>>> {
        let partially_staged_files = self.get_partially_staged_files(true)?;

        if partially_staged_files.is_empty() {
            return Ok(None);
        }

        let mut diff_options = DiffOptions::new();
        diff_options.show_binary(true);
        for file in partially_staged_files.iter() {
            diff_options.pathspec(file);
        }

        let unstaged_diff = self
            .repository
            .diff_index_to_workdir(None, Some(&mut diff_options))?;

        let mut unstaged_diff_buffer = vec![];
        unstaged_diff.print(DiffFormat::Patch, |_, _, line| {
            let origin = line.origin();

            if origin == '+' || origin == '-' || origin == ' ' {
                unstaged_diff_buffer.push(origin as u8);
            }

            unstaged_diff_buffer.append(&mut line.content().to_vec());
            true
        })?;

        Ok(Some(unstaged_diff_buffer))
    }

    fn hide_partially_staged_changes(&self) -> Result<()> {
        let partially_staged_files = self.get_partially_staged_files(false)?;

        let mut checkout_options = CheckoutBuilder::new();
        checkout_options.force();
        checkout_options.update_index(false);
        for file in partially_staged_files.iter() {
            checkout_options.path(file);
        }

        self.repository
            .checkout_index(None, Some(&mut checkout_options))?;

        Ok(())
    }

    pub fn get_staged_files(&self) -> Result<Vec<PathBuf>> {
        let head_tree = match self.repository.head() {
            Ok(head) => Ok(Some(head.peel_to_tree()?)),
            Err(error) if error.code() == ErrorCode::UnbornBranch => Ok(None),
            Err(error) => Err(error),
        }?;

        let staged_files = self
            .repository
            .diff_tree_to_index(head_tree.as_ref(), None, None)?
            .deltas()
            .flat_map(|delta| {
                if delta.old_file().path() == delta.new_file().path() {
                    vec![delta.old_file().path()]
                } else {
                    vec![delta.old_file().path(), delta.new_file().path()]
                }
            })
            .filter_map(std::convert::identity)
            .map(Path::to_path_buf)
            .collect();

        Ok(staged_files)
    }

    fn get_partially_staged_files(&self, include_from_files: bool) -> Result<HashSet<PathBuf>> {
        let staged_files = HashSet::from_iter(self.get_staged_files()?);

        let unstaged_files = HashSet::from_iter(
            self.repository
                .diff_index_to_workdir(None, Some(DiffOptions::default().show_binary(true)))?
                .deltas()
                .flat_map(|delta| {
                    if include_from_files {
                        vec![delta.old_file().path(), delta.new_file().path()]
                    } else {
                        vec![delta.new_file().path()]
                    }
                })
                .filter_map(std::convert::identity)
                .map(Path::to_path_buf),
        );

        fn intersect<P: Eq + Hash>(one: HashSet<P>, two: &HashSet<P>) -> HashSet<P> {
            one.into_iter().filter(|p| two.contains(p)).collect()
        }

        Ok(intersect(staged_files, &unstaged_files))
    }

    fn get_deleted_files(&self) -> Result<Vec<PathBuf>> {
        let deleted_files = self
            .repository
            .diff_index_to_workdir(None, None)?
            .deltas()
            .filter(|delta| delta.status() == Delta::Deleted)
            .filter_map(|delta| delta.old_file().path())
            .map(Path::to_path_buf)
            .collect_vec();

        Ok(deleted_files)
    }

    fn save_snapshot_stash(&mut self) -> Result<Option<Stash>> {
        if self.repository.is_empty()? {
            return Ok(None);
        }

        fn create_signature<'a>() -> Result<Signature<'a>> {
            // Because this time is only used to create a dummy signature to
            // make the stash_save method happy, we don't need to use a real
            // time, which skips some calls to the kernel.
            //
            let time = Time::new(0, 0);

            Signature::new("Dummy", "dummy@example.com", &time)
                .with_context(|| "Encountered an error when creating dummy authorship information.")
        }

        // Save state when in the middle of a merge prior to stashing changes in
        // the working directory so that we can restore it afterward.
        //
        let merge_status = self.save_merge_status()?;

        let signature = create_signature()?;

        let stash_result = self
            .repository
            .stash_save(&signature, "offstage backup", None);

        // Until save_snapshot_stash can use a non-destructive stash (which maps
        // to command `git stash create` and `git stash store`), which needs to
        // be supported by libgit2, we need to apply the stash to bring back files.
        //
        if let Ok(stash_id) = stash_result {
            self.apply_stash(&stash_id)?;
            self.restore_merge_status(&merge_status)?;
        }

        match stash_result {
            Ok(stash_id) => Ok(Some(Stash {
                stash_id,
                merge_status,
            })),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(anyhow!(error)
                .context("Encountered an error when stashing a backup of the working directory.")),
        }
    }

    fn save_merge_status(&self) -> Result<MergeStatus> {
        let merge_head_path = &self.repository.path().join("MERGE_HEAD");
        let merge_head = Self::read_file_to_string(merge_head_path).with_context(|| {
            format!(
                "Encountered an error when saving {}.",
                merge_head_path.display()
            )
        })?;

        let merge_mode_path = &self.repository.path().join("MERGE_MODE");
        let merge_mode = Self::read_file_to_string(merge_mode_path).with_context(|| {
            format!(
                "Encountered an error when saving {}.",
                merge_mode_path.display()
            )
        })?;

        let merge_msg_path = &self.repository.path().join("MERGE_MSG");
        let merge_msg = Self::read_file_to_string(merge_msg_path).with_context(|| {
            format!(
                "Encountered an error when saving {}.",
                merge_msg_path.display()
            )
        })?;

        Ok(MergeStatus {
            merge_head,
            merge_mode,
            merge_msg,
        })
    }

    fn restore_merge_status(&self, merge_status: &MergeStatus) -> Result<()> {
        // Tries to restore all files before returning the first error if one exists.

        let restore_merge_head_result =
            merge_status
                .merge_head
                .as_ref()
                .map_or(Ok(()), |merge_head| {
                    let merge_head_path = &self.repository.path().join("MERGE_HEAD");
                    fs::write(merge_head_path, merge_head).with_context(|| {
                        format!(
                            "Encountered an error when restoring {}.",
                            merge_head_path.display()
                        )
                    })
                });

        let restore_merge_mode_result =
            merge_status
                .merge_mode
                .as_ref()
                .map_or(Ok(()), |merge_mode| {
                    let merge_mode_path = &self.repository.path().join("MERGE_MODE");
                    fs::write(merge_mode_path, merge_mode).with_context(|| {
                        format!(
                            "Encountered an error when restoring {}.",
                            &merge_mode_path.display()
                        )
                    })
                });

        let restore_merge_msg_result =
            merge_status.merge_msg.as_ref().map_or(Ok(()), |merge_msg| {
                let merge_msg_path = &self.repository.path().join("MERGE_MSG");
                fs::write(merge_msg_path, merge_msg).with_context(|| {
                    format!(
                        "Encountered an error when restoring {}.",
                        merge_msg_path.display()
                    )
                })
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
            fs::remove_file(file).with_context(|| {
                format!(
                    "Encountered error when deleting {}.",
                    file.as_ref().display()
                )
            })?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Snapshot {
    pub staged_files: Vec<PathBuf>,
    backup_stash: Option<Stash>,
    unstaged_diff: Option<Vec<u8>>,
}

#[derive(Debug)]
struct Stash {
    stash_id: Oid,
    merge_status: MergeStatus,
}

#[derive(Debug)]
struct MergeStatus {
    merge_head: Option<String>,
    merge_mode: Option<String>,
    merge_msg: Option<String>,
}
