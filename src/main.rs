use std::path::PathBuf;
use std::process;
use std::process::Command;
use structopt::StructOpt;
use git::GitRepository;

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

fn main() {
    let args = Args::from_args();

    let mut repository = GitRepository::open();

    let snapshot = repository.save_snapshot_stash();

    println!("Snapshot: {:?}", snapshot);

    let status = Command::new(&args.shell)
        .arg("-c")
        .arg(join_command(&args))
        .status()
        .unwrap();

    process::exit(status.code().unwrap_or(1));
}

fn join_command(args: &Args) -> String {
    args.command.join(" ")
}