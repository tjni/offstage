use anyhow::Result;
use std::path::PathBuf;
use structopt::StructOpt;

mod git;
mod workflow;

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

    workflow::run(&args.shell, &args.command)
}