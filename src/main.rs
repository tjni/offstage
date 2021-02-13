use anyhow::Result;
use git::{GitWorkflow, Snapshot};
use std::path::PathBuf;
use std::process::Command;
use structopt::StructOpt;

mod git;

#[derive(Debug, StructOpt)]
struct Args {
    /// File filter over staged files
    #[structopt(long, short)]
    filter: Option<String>,

    /// Shell executable to use to run the command
    #[structopt(long, short, env = "SHELL")]
    shell: PathBuf,

    /// Command to run on staged files
    command: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::from_args();

    let mut workflow = GitWorkflow::open()?;

    let snapshot = workflow.save_snapshot()?;

    let result = run_stage_command(&args, &mut workflow, &snapshot);

    if let Some(_) = result.err() {
        workflow.restore_snapshot(snapshot)
    } else {
        Ok(())
    }
}

fn run_stage_command(args: &Args, workflow: &mut GitWorkflow, snapshot: &Snapshot) -> Result<()> {
    let status = Command::new(&args.shell)
        .arg("-c")
        .arg(join_command_pieces(&args))
        .status()?;

    if status.code().unwrap_or(1) == 0 {
        workflow.apply_modifications(snapshot)
    } else {
        Ok(())
    }
}

fn join_command_pieces(args: &Args) -> String {
    args.command.join(" ")
}
