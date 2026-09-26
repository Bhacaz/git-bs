mod git;
mod picker;
mod ui;

use git::Repository;
use picker::Picker;
use std::{
    env,
    io::{self, IsTerminal},
    process::ExitCode,
};

const HELP: &str = "git-bs — interactive, searchable local branch checkout

Usage: git bs [--query TEXT] [--list]
       git-bs [--query TEXT] [--list]

Options:
  -q, --query TEXT  Start with a branch-name search
  -l, --list        Print matching branches without opening the picker
  -h, --help        Show this help
  -V, --version     Show the version

Keys:
  Type to search; Up/Down or Ctrl-P/Ctrl-N to navigate
  PageUp/PageDown to scroll; Home/End for the first/last branch
  Backspace to erase; Ctrl-U to clear the search
  Enter to checkout; Esc or Ctrl-C to cancel

Branches are sorted by last visit, then newest commit for unvisited branches.
Search uses fuzzy subsequences,
smart case, and space-separated AND terms. Requires Git, but not fzf.
";

fn run() -> io::Result<ExitCode> {
    let mut args = env::args().skip(1);
    let mut query = String::new();
    let mut list = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(ExitCode::SUCCESS);
            }
            "-V" | "--version" => {
                println!("git-bs {}", env!("CARGO_PKG_VERSION"));
                return Ok(ExitCode::SUCCESS);
            }
            "-l" | "--list" => list = true,
            "-q" | "--query" => {
                query = args
                    .next()
                    .ok_or_else(|| io::Error::other("--query requires text"))?
            }
            _ => {
                return Err(io::Error::other(format!(
                    "Unknown argument: {arg}\nRun git-bs --help for usage."
                )));
            }
        }
    }
    let repo = Repository::new(env::current_dir()?);
    let branches = repo.branches()?;
    if branches.is_empty() {
        println!("No local branches found. Create a commit to get started.");
        return Ok(ExitCode::SUCCESS);
    }
    let mut picker = Picker::new(branches, query);
    if list {
        for matched in &picker.filtered {
            let branch = &picker.branches[matched.index];
            println!(
                "{} {:<50} ({})",
                if branch.current { '*' } else { ' ' },
                branch.name,
                branch.age
            );
        }
        return Ok(ExitCode::SUCCESS);
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other(
            "Interactive mode requires a terminal. Use --list to print branches.",
        ));
    }
    if !ui::select(&repo, &mut picker)? {
        println!("No branch selected. Exiting.");
        return Ok(ExitCode::SUCCESS);
    }
    let branch = picker
        .selected_branch()
        .expect("accepted selection must exist");
    if branch.current {
        println!("Already on branch '{}'", branch.name);
        return Ok(ExitCode::SUCCESS);
    }
    println!("Checking out branch: {}", branch.name);
    let status = repo.checkout(branch)?;
    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(
            status
                .code()
                .and_then(|c| u8::try_from(c).ok())
                .unwrap_or(1),
        )
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("git-bs: {error}");
            ExitCode::FAILURE
        }
    }
}
