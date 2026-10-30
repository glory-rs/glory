"""Prepare ignored library locks once, including independent example workspaces."""
from pathlib import Path
import argparse
import json
import os
import subprocess
import tomllib


def ensure_library_lockfiles(root=None):
    root = Path(root or Path(__file__).resolve().parents[1]).resolve()
    configured = os.environ.get("CARGO_TARGET_DIR")
    if not configured:
        raise SystemExit("Set CARGO_TARGET_DIR before resolving library dependencies")
    target = Path(configured).resolve()
    if os.name == "nt" and not target.is_relative_to(Path("D:/BuildCache").resolve()):
        raise SystemExit("Build artifacts must stay under D:/BuildCache")
    tracked = subprocess.check_output(["git", "-C", str(root), "ls-files", "*Cargo.toml"], text=True).splitlines()
    manifests = {root / "Cargo.toml"}
    for relative in tracked:
        manifest = root / relative
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        if "workspace" in data:
            manifests.add(manifest)
            for excluded in data["workspace"].get("exclude", []):
                candidate = manifest.parent / excluded / "Cargo.toml"
                if candidate.is_file():
                    manifests.add(candidate)
    for manifest in sorted(manifests):
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)],
            cwd=root, text=True, encoding="utf-8"))
        if Path(metadata["target_directory"]).resolve() != target:
            raise SystemExit(f"Unexpected Cargo target for {manifest}")
        lock = Path(metadata["workspace_root"]) / "Cargo.lock"
        if not lock.exists():
            subprocess.run(["cargo", "--config", 'resolver.incompatible-rust-versions="fallback"',
                            "generate-lockfile", "--manifest-path", str(manifest)], cwd=root, check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path)
    ensure_library_lockfiles(parser.parse_args().root)
