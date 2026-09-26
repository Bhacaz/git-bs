#!/usr/bin/env python3
"""Check installer first install, repeat, update, and rejected checksums."""
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import threading

ROOT = Path(__file__).resolve().parents[1]


def fixture(directory, tag, *, bad_checksum=False):
    release = directory / tag
    release.mkdir()
    binary = f'#!/bin/sh\nif [ "$1" = "--version" ]; then echo "git-bs {tag[1:]}"; fi\n'.encode()
    checksums = []
    for platform in ("linux-x86_64", "linux-arm64"):
        archive = release / f"git-bs-{tag}-{platform}.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            info = tarfile.TarInfo("git-bs")
            info.mode = 0o755
            info.size = len(binary)
            tar.addfile(info, io.BytesIO(binary))
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if bad_checksum:
            digest = "0" * 64
        checksums.append(f"{digest}  ./{archive.name}\n")
    (release / "SHA256SUMS").write_text("".join(checksums))


with tempfile.TemporaryDirectory(prefix="git-bs-installer-test-") as temporary:
    base = Path(temporary)
    fixtures = base / "releases"
    fixtures.mkdir()
    for tag in ("v0.2.0", "v0.2.1"):
        fixture(fixtures, tag)
    fixture(fixtures, "v0.2.2", bad_checksum=True)
    state = {"latest": "v0.2.0"}

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path == "/releases/latest":
                self.send_response(302)
                self.send_header("Location", f"/releases/tag/{state['latest']}")
                self.end_headers()
                return
            if self.path.startswith("/releases/tag/"):
                self.send_response(200)
                self.end_headers()
                return
            parts = self.path.split("/")
            if len(parts) != 5 or parts[1:3] != ["releases", "download"]:
                self.send_error(404)
                return
            file = fixtures / parts[3] / parts[4]
            if not file.is_file():
                self.send_error(404)
                return
            data = file.read_bytes()
            self.send_response(200)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def log_message(self, _format, *_args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    environment = {
        **os.environ,
        "GIT_BS_RELEASE_BASE_URL": f"http://127.0.0.1:{server.server_port}/releases",
        "GIT_BS_INSTALL_DIR": str(base / "bin with spaces and apostrophe's"),
        "GIT_CONFIG_GLOBAL": str(base / "gitconfig"),
    }

    def run():
        return subprocess.run(["bash", str(ROOT / "install.sh")], env=environment,
                              capture_output=True, text=True)

    def git_version():
        return subprocess.check_output(["git", "bs", "--version"], env=environment,
                                       cwd=base, text=True).strip()

    try:
        first = run()
        assert first.returncode == 0, first.stderr
        assert "Installed git-bs 0.2.0" in first.stdout
        assert git_version() == "git-bs 0.2.0"

        repeat = run()
        assert repeat.returncode == 0, repeat.stderr
        assert "Already up to date" in repeat.stdout

        state["latest"] = "v0.2.1"
        update = run()
        assert update.returncode == 0, update.stderr
        assert "Updated git-bs 0.2.1" in update.stdout
        assert git_version() == "git-bs 0.2.1"

        state["latest"] = "v0.2.2"
        rejected = run()
        assert rejected.returncode != 0 and "Checksum verification failed" in rejected.stderr
        assert git_version() == "git-bs 0.2.1"

        state["latest"] = "v0.2.1"
        fake_commands = base / "fake-commands"
        fake_commands.mkdir()
        fake_uname = fake_commands / "uname"
        fake_uname.write_text('#!/bin/sh\nif [ "$1" = "-m" ]; then echo aarch64; else /usr/bin/uname "$@"; fi\n')
        fake_uname.chmod(0o755)
        arm_environment = {
            **environment,
            "PATH": f"{fake_commands}:{environment['PATH']}",
            "GIT_BS_INSTALL_DIR": str(base / "arm-bin"),
            "GIT_CONFIG_GLOBAL": str(base / "arm-gitconfig"),
        }
        arm = subprocess.run(["bash", str(ROOT / "install.sh")], env=arm_environment,
                             capture_output=True, text=True)
        assert arm.returncode == 0, arm.stderr
        assert subprocess.check_output(["git", "bs", "--version"], env=arm_environment,
                                       cwd=base, text=True).strip() == "git-bs 0.2.1"
    finally:
        server.shutdown()
        thread.join()

print("Installer smoke test passed: x86-64/ARM64, install, update, checksum rejection, and Git alias.")
