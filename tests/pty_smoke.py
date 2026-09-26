#!/usr/bin/env python3
"""Exercise the actual executable and terminal lifecycle in isolated repositories."""
import errno
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time

BINARY = str(Path(sys.argv[1] if len(sys.argv) > 1 else "target/release/git-bs").resolve())


def git(directory, *args):
    return subprocess.check_output(["git", *args], cwd=directory, stderr=subprocess.PIPE).decode().strip()


def interact(directory, keys, args=()):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
    original = termios.tcgetattr(slave)
    process = subprocess.Popen([BINARY, *args], cwd=directory, stdin=slave, stdout=slave, stderr=slave,
                               env={**os.environ, "TERM": "xterm-256color"})
    output = bytearray()
    sent = False
    deadline = time.monotonic() + 10
    try:
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    output.extend(os.read(master, 65536))
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
            if not sent and b"Last 15 commits" in output:
                os.write(master, keys)
                sent = True
            if process.poll() is not None:
                while select.select([master], [], [], 0)[0]:
                    output.extend(os.read(master, 65536))
                break
        else:
            raise AssertionError(f"Timed out: {output!r}")
        assert sent, output
        assert termios.tcgetattr(slave) == original, "Terminal settings were not restored"
        assert b"\x1b[?1049l" in output, "Alternate screen was not restored"
        return process.returncode, output.decode(errors="replace")
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        os.close(master)
        os.close(slave)


with tempfile.TemporaryDirectory(prefix="git-bs-smoke-") as directory:
    git(directory, "init", "-b", "main")
    git(directory, "config", "user.name", "Test")
    git(directory, "config", "user.email", "test@example.com")
    git(directory, "config", "commit.gpgsign", "false")
    file = Path(directory) / "tracked.txt"
    file.write_text("main\n")
    git(directory, "add", ".")
    git(directory, "commit", "-m", "Main commit")
    git(directory, "checkout", "-b", "feature/login")
    file.write_text("feature\n")
    git(directory, "commit", "-am", "Feature commit")
    git(directory, "checkout", "main")

    code, output = interact(directory, b"ftlg\r")
    assert code == 0 and "Checking out branch: feature/login" in output
    assert git(directory, "branch", "--show-current") == "feature/login"
    print("PASS: fuzzy typing selects and checks out a branch")

    code, output = interact(directory, b"\r", ["--query", "feature/login"])
    assert code == 0 and "Already on branch 'feature/login'" in output
    print("PASS: current branch is a no-op")

    for keys in (b"main\x1b", b"main\x03", b"nonexistent\r\x1b"):
        code, output = interact(directory, keys)
        assert code == 0 and "No branch selected" in output
        assert git(directory, "branch", "--show-current") == "feature/login"
    print("PASS: Escape, Ctrl-C, and Enter with no matches do not change branches")

    # End selects main (the second alphabetical ref when commit timestamps tie).
    # Use Ctrl-U to clear an initial query and Down to reach the second result.
    lines = git(directory, "for-each-ref", "--sort=-committerdate", "--format=%(refname:short)", "refs/heads/").splitlines()
    git(directory, "checkout", lines[0])
    code, output = interact(directory, b"\x15\x1b[B\r", ["--query", "nonexistent"])
    assert code == 0 and git(directory, "branch", "--show-current") == lines[1]
    print("PASS: clear search and arrow-key navigation")

    git(directory, "checkout", "main")
    file.write_text("uncommitted work\n")
    code, output = interact(directory, b"\r", ["--query", "feature/login"])
    assert code != 0 and "overwritten by checkout" in output
    assert git(directory, "branch", "--show-current") == "main"
    assert file.read_text() == "uncommitted work\n"
    print("PASS: dirty-worktree checkout fails without losing changes")

    file.write_text("main\n")
    git(directory, "checkout", "--detach")
    code, output = interact(directory, b"main\r")
    assert code == 0 and git(directory, "branch", "--show-current") == "main"
    print("PASS: checkout from detached HEAD")

    nested = Path(directory) / "nested"
    nested.mkdir()
    code, output = interact(nested, b"main\r")
    assert code == 0 and "Already on branch 'main'" in output
    print("PASS: repository subdirectory")

    with tempfile.TemporaryDirectory(prefix="git-bs-worktree-") as worktree_parent:
        worktree = str(Path(worktree_parent) / "checkout")
        git(directory, "worktree", "add", worktree, "feature/login")
        code, output = interact(worktree, b"\r", ["--query", "feature/login"])
        assert code == 0 and "Already on branch 'feature/login'" in output
        code, output = interact(directory, b"\r", ["--query", "feature/login"])
        assert code != 0 and git(directory, "branch", "--show-current") == "main"
        git(directory, "worktree", "remove", worktree)
    print("PASS: linked worktrees and Git's existing-worktree checkout protection")

print("All pseudo-terminal checks passed; terminal settings restored after every interaction.")
