use std::{
    io,
    path::PathBuf,
    process::{Command, ExitStatus},
};

#[derive(Clone, Debug)]
pub struct Branch {
    pub name: String,
    pub age: String,
    pub current: bool,
}

pub struct Repository {
    directory: PathBuf,
}

impl Repository {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command.current_dir(&self.directory);
        command
    }

    fn output(&self, args: &[&str]) -> io::Result<String> {
        let output = self.command().args(args).output().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    error.kind(),
                    "Git was not found. Install Git and put it on PATH.",
                )
            } else {
                error
            }
        })?;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        // Never silently turn a non-UTF-8 ref into a different branch name.
        String::from_utf8(output.stdout).map_err(|_| {
            io::Error::other("Git returned non-UTF-8 text; branch names must be UTF-8.")
        })
    }

    pub fn branches(&self) -> io::Result<Vec<Branch>> {
        let output = self.output(&[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname)%00%(committerdate:relative)%00%(HEAD)",
            "refs/heads/",
        ])?;
        output
            .lines()
            .map(|line| {
                let mut fields = line.split('\0');
                let name = fields
                    .next()
                    .and_then(|name| name.strip_prefix("refs/heads/"));
                match (name, fields.next(), fields.next()) {
                    (Some(name), Some(age), Some(head)) => Ok(Branch {
                        name: name.to_string(),
                        age: age.to_string(),
                        current: head == "*",
                    }),
                    _ => Err(io::Error::other("Unexpected branch data from Git.")),
                }
            })
            .collect()
    }

    pub fn preview(&self, branch: &Branch) -> io::Result<String> {
        // Fully qualify refs so tags and filenames cannot shadow local branches.
        self.output(&[
            "--no-pager",
            "log",
            "--oneline",
            "--graph",
            "--color=never",
            "--no-show-signature",
            "-15",
            &format!("refs/heads/{}", branch.name),
            "--",
        ])
        .map(|text| {
            text.chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .collect()
        })
    }

    pub fn checkout(&self, branch: &Branch) -> io::Result<ExitStatus> {
        // Inherit Git's output and preserve its normal dirty-worktree protections.
        self.command()
            .args(["checkout", "--no-guess", &branch.name, "--"])
            .status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn git(dir: &TempDir, args: &[&str]) {
        assert!(
            Command::new("git")
                .current_dir(dir.path())
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }

    #[test]
    fn local_branches_preview_and_checkout() {
        let dir = TempDir::new().unwrap();
        git(&dir, &["init", "-b", "main"]);
        git(
            &dir,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "--allow-empty",
                "-m",
                "first | subject",
            ],
        );
        git(&dir, &["branch", "feature/été"]);
        git(&dir, &["tag", "feature/été"]);
        git(
            &dir,
            &["update-ref", "refs/remotes/origin/remote-only", "HEAD"],
        );
        let repo = Repository::new(dir.path().into());
        let branches = repo.branches().unwrap();
        assert_eq!(branches.len(), 2);
        assert!(branches.iter().find(|b| b.name == "main").unwrap().current);
        let feature = branches.iter().find(|b| b.name == "feature/été").unwrap();
        assert!(repo.preview(feature).unwrap().contains("first | subject"));
        assert!(repo.checkout(feature).unwrap().success());
        assert!(
            repo.branches()
                .unwrap()
                .iter()
                .find(|b| b.name == feature.name)
                .unwrap()
                .current
        );
        git(&dir, &["checkout", "--detach"]);
        assert!(repo.branches().unwrap().iter().all(|b| !b.current));
    }

    #[test]
    fn empty_repository_and_non_repository() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::new(dir.path().into());
        assert!(repo.branches().is_err());
        git(&dir, &["init"]);
        assert!(repo.branches().unwrap().is_empty());
    }

    #[test]
    fn preview_is_limited_to_fifteen_commits() {
        let dir = TempDir::new().unwrap();
        git(&dir, &["init", "-b", "main"]);
        for number in 0..20 {
            git(
                &dir,
                &[
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "--allow-empty",
                    "-m",
                    &format!("commit-{number:02}"),
                ],
            );
        }
        let repo = Repository::new(dir.path().into());
        let preview = repo.preview(&repo.branches().unwrap()[0]).unwrap();
        assert_eq!(preview.lines().count(), 15);
        assert!(preview.contains("commit-19"));
        assert!(preview.contains("commit-05"));
        assert!(!preview.contains("commit-04"));
    }
}
