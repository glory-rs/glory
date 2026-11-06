"""Prepare ignored locks for explicitly maintained Glory workspaces."""
from pathlib import Path
import argparse
import json
import os
import subprocess
import tomllib

# Consumer repositories and peer-framework benchmarks are deliberately outside
# the default build/audit boundary. Add maintained upstream workspaces explicitly.
LIBRARY_MANIFESTS = (
    "Cargo.toml", "tools/cli/Cargo.toml",
    "examples/counter/Cargo.toml", "examples/display-web/Cargo.toml",
    "examples/mobile/Cargo.toml", "examples/routing/Cargo.toml",
    "examples/semantic-web/Cargo.toml", "benchmarks/glory/Cargo.toml",
    "benchmarks/role-patches/Cargo.toml", "benchmarks/wasm-size/Cargo.toml",
    "benchmarks/scenarios/glory/Cargo.toml",
)


def ensure_library_lockfiles(root=None, manifests=None):
    root = Path(root or Path(__file__).resolve().parents[1]).resolve()
    configured = os.environ.get("CARGO_TARGET_DIR")
    target = Path(configured).resolve() if configured else None
    pass  # Build paths follow the caller configuration.
    selected = set()
    for relative in LIBRARY_MANIFESTS if manifests is None else manifests:
        manifest = (root / relative).resolve()
        if not manifest.is_relative_to(root) or not manifest.is_file():
            raise SystemExit(f"Expected a repository-owned manifest: {relative}")
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        if "workspace" not in data:
            raise SystemExit(f"Expected an independent workspace: {relative}")
        selected.add(manifest)
    for manifest in sorted(selected):
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)],
            cwd=manifest.parent, text=True, encoding="utf-8"))
        if target is not None and Path(metadata["target_directory"]).resolve() != target:
            raise SystemExit(f"Unexpected Cargo target for {manifest}")
        lock = Path(metadata["workspace_root"]) / "Cargo.lock"
        if not lock.exists():
            subprocess.run(["cargo", "--config", 'resolver.incompatible-rust-versions="fallback"',
                            "generate-lockfile", "--manifest-path", str(manifest)], cwd=manifest.parent, check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--manifest-path", action="append", help="Resolve only this repository-owned workspace (repeatable)")
    options = parser.parse_args()
    ensure_library_lockfiles(options.root, options.manifest_path)
