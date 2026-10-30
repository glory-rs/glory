"""Resolve untracked library locks once, then keep --locked checks stable."""
from pathlib import Path
import subprocess


def ensure_library_lockfiles(root=None):
    root = Path(root) if root is not None else Path(__file__).resolve().parents[1]
    for relative in ("Cargo.toml", "tools/cli/Cargo.toml"):
        manifest = root / relative
        if not manifest.with_name("Cargo.lock").exists():
            subprocess.run(
                ["cargo", "--config", 'resolver.incompatible-rust-versions="fallback"',
                 "generate-lockfile", "--manifest-path", str(manifest)],
                cwd=root, check=True,
            )


if __name__ == "__main__":
    ensure_library_lockfiles()
