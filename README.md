# git-bs

A standalone Rust executable for interactive, searchable Git branch checkout,
based on [git-branch-select.sh](https://gist.github.com/Bhacaz/c00f7174918c00b5ab3a5af080fece04).
The picker is built into the executable; **fzf and Bash are not required**.
Git must be installed and available on `PATH`.

## Build and run

Requires Rust 1.88 or newer to build. The compiled executable does not need Rust
or Cargo installed to run.

```sh
cargo build --release --locked
./target/release/git-bs --help
```

Run the executable from any directory inside a Git repository. The delivered
Linux build is also available as `dist/git-bs`.

To install from source:

```sh
cargo install --path . --locked
```

With Cargo's bin directory (`~/.cargo/bin` by default) on your `PATH`, run:

```sh
git bs
# or
git-bs
```

Git discovers the `git-bs` executable automatically; no alias is needed. If you
already configured the gist's `bs` alias, it takes precedence. Remove that old
alias from the configuration where you added it (for a global alias,
`git config --global --unset alias.bs`), or invoke `git-bs` directly.

You can also copy the release executable into a directory on your `PATH`, such
as `~/.local/bin`. Builds are specific to the target OS and architecture; use
Cargo on the destination platform to build for another system.

## Behavior

- Lists local branches, newest committer date first, with relative ages.
- Marks the current branch with a green `*`.
- Searches branch names while retaining their commit-date order.
- Shows a graph of the selected branch's last 15 commits.
- Checks out the selected branch using Git's usual worktree protections.
- Selecting the current branch prints an acknowledgment without running checkout.
- Canceling makes no changes and exits successfully.
- Works from repository subdirectories, linked worktrees, and detached HEAD.
- Empty repositories and non-terminal invocations receive readable messages.

The interface uses the terminal's alternate screen and restores the previous
screen on exit. Wide terminals use a 70/30 branch/preview split; narrower
terminals stack the preview below the list. This replaces the gist's 40%-height
fzf interface with a full-screen native picker.

Search matches characters in order: `ftlg` matches `feature/login`. Lowercase
queries ignore case; queries containing uppercase letters match case exactly.
Space-separated terms must all match. Only branch names are searched, not ages
or commit messages. This implements ordinary fuzzy search, not fzf's advanced
operator syntax (`!`, `^`, `$`, `|`, or quoted exact matches).

| Key | Action |
| --- | --- |
| Type or paste | Search branch names |
| ↑ / ↓ or Ctrl-P / Ctrl-N | Select previous / next branch |
| PageUp / PageDown | Move one page through branches |
| Home / End | Select first / last match |
| Backspace | Remove the last search character |
| Ctrl-U | Clear the search |
| Enter | Check out the selected branch |
| Esc / Ctrl-C | Cancel |

```sh
git bs --query feature   # Pre-fill the search
git bs --list            # Print branches without opening the UI
git bs --list -q fix     # Print matching branches, without checking out
git bs --help
git bs --version
```

Exit status is zero for a successful selection, cancellation, or an empty
repository. Argument, repository, and terminal errors return a nonzero status;
checkout failures preserve Git's exit code. No force checkout, automatic stash,
branch deletion, or remote operations are performed. Branch names must be UTF-8.

## Development and verification

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
python3 tests/pty_smoke.py target/release/git-bs
```

The optional Python smoke test uses a real pseudo-terminal on Linux/macOS and
temporary Git repositories to exercise typing, navigation, checkout,
cancellation, terminal restoration, and refusal to overwrite dirty files.
Python is not a runtime dependency of the application.

