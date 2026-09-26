use std::process::{Command, Output};
use tempfile::TempDir;

fn run(dir: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_git-bs"))
        .current_dir(dir.path())
        .args(args)
        .output()
        .unwrap()
}

fn git(dir: &TempDir, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(dir.path())
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn commit(dir: &TempDir, date: &str) {
    assert!(
        Command::new("git")
            .current_dir(dir.path())
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "--allow-empty",
                "-m",
                "commit"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn help_version_and_invalid_arguments_outside_a_repository() {
    let dir = TempDir::new().unwrap();
    assert!(run(&dir, &["--help"]).status.success());
    assert!(
        String::from_utf8_lossy(&run(&dir, &["--version"]).stdout)
            .contains(env!("CARGO_PKG_VERSION"))
    );
    assert!(!run(&dir, &["--unknown"]).status.success());
    assert!(!run(&dir, &["--query"]).status.success());
}

#[test]
fn lists_only_local_branches_last_visited_first_and_searches_names() {
    let dir = TempDir::new().unwrap();
    git(&dir, &["init", "-b", "main"]);
    commit(&dir, "2020-01-01T12:00:00Z");
    git(&dir, &["branch", "aardvark"]);
    git(&dir, &["branch", "alpha"]);
    commit(&dir, "2024-01-01T12:00:00Z");
    git(&dir, &["branch", "never"]);
    git(&dir, &["checkout", "-b", "feature/new"]);
    commit(&dir, "2025-01-01T12:00:00Z");
    git(&dir, &["checkout", "main"]);
    git(&dir, &["checkout", "alpha"]);
    git(
        &dir,
        &["update-ref", "refs/remotes/origin/remote-only", "HEAD"],
    );
    let output = run(&dir, &["--list"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 5);
    assert!(lines[0].starts_with("* alpha"));
    assert!(lines[1].starts_with("  main"));
    assert!(lines[2].starts_with("  feature/new"));
    assert!(lines[3].starts_with("  never"));
    assert!(lines[4].starts_with("  aardvark"));
    let filtered = run(&dir, &["--list", "-q", "fnw"]);
    assert!(String::from_utf8_lossy(&filtered.stdout).contains("feature/new"));
    assert!(!String::from_utf8_lossy(&filtered.stdout).contains("main"));
    assert!(run(&dir, &["--list", "-q", "ago"]).stdout.is_empty());
}

#[test]
fn handles_empty_and_non_repositories() {
    let dir = TempDir::new().unwrap();
    assert!(!run(&dir, &["--list"]).status.success());
    git(&dir, &["init"]);
    let output = run(&dir, &[]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("No local branches"));
}

#[test]
fn non_interactive_mode_requires_list_flag() {
    let dir = TempDir::new().unwrap();
    git(&dir, &["init", "-b", "main"]);
    commit(&dir, "2020-01-01T12:00:00Z");
    let output = run(&dir, &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires a terminal"));
}
