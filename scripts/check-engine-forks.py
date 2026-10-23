"""Verify immutable engine Git dependencies; optionally test clean fork checkouts."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tomllib


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def output(*args):
    return run(*args, stdout=subprocess.PIPE).stdout.strip()


def build_environment():
    paths = {}
    for name in ("CARGO_HOME", "CARGO_TARGET_DIR"):
        value = Path(os.environ[name]).resolve()
        portable = value.as_posix().lower()
        assert portable.startswith(("d:/buildcache/", "/mnt/d/buildcache/")), (name, value)
        paths[name] = value
    return paths


def test_fork(fork, directory, target):
    if not directory.exists():
        run("git", "clone", "--filter=blob:none", "--no-checkout", fork["git"], str(directory))
    else:
        assert not output("git", "-C", str(directory), "status", "--porcelain"), directory
    assert output("git", "-C", str(directory), "remote", "get-url", "origin") == fork["git"]
    available = subprocess.run(
        ["git", "-C", str(directory), "cat-file", "-e", fork["rev"] + "^{commit}"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    if available.returncode:
        run("git", "-C", str(directory), "fetch", "origin", fork["rev"])
    run("git", "-C", str(directory), "checkout", "--detach", fork["rev"])
    manifest = str(directory / "Cargo.toml")
    metadata = json.loads(output("cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", manifest))
    assert Path(metadata["target_directory"]).resolve() == target
    for arguments in [fork["tests"], *fork.get("extra_tests", [])]:
        run("cargo", "test", "--locked", "--manifest-path", manifest, *arguments)
    assert not output("git", "-C", str(directory), "status", "--porcelain"), directory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test", action="store_true", help="also run engine unit tests in separate Git checkouts under the D: build root")
    options = parser.parse_args()
    paths = build_environment()
    root = Path(__file__).resolve().parents[1]
    record = json.loads((root / "docs/engine-forks.json").read_text(encoding="utf-8"))
    metadata = json.loads(output("cargo", "metadata", "--locked", "--all-features", "--format-version", "1", "--manifest-path", str(root / "Cargo.toml")))
    assert Path(metadata["target_directory"]).resolve() == paths["CARGO_TARGET_DIR"]
    assert len(metadata["workspace_members"]) == 10
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    for fork in record["forks"]:
        assert len(fork["rev"]) == 40 and all(c in "0123456789abcdef" for c in fork["rev"])
        source = f"git+{fork['git']}?rev={fork['rev']}#{fork['rev']}"
        for name in fork["packages"]:
            resolved = [package for package in metadata["packages"] if package["name"] == name]
            assert len(resolved) == 1 and resolved[0]["source"] == source, (name, resolved)
            locked = [package for package in lock["package"] if package["name"] == name]
            assert len(locked) == 1 and locked[0].get("source") == source, (name, locked)
        if options.test:
            directory = paths["CARGO_TARGET_DIR"].parent / "engine-forks" / fork["name"]
            directory.parent.mkdir(parents=True, exist_ok=True)
            test_fork(fork, directory, paths["CARGO_TARGET_DIR"])
    retired = {"glory-blitz-dom", "glory-blitz-traits", "glory-blitz-paint", "glory-blitz-shell", "glory-taffy", "glory-stylo-taffy"}
    for lockfile in (root / "Cargo.lock", root / "examples/semantic-web/Cargo.lock"):
        packages = tomllib.loads(lockfile.read_text(encoding="utf-8"))["package"]
        assert not retired.intersection(package["name"] for package in packages), lockfile
        for fork in record["forks"]:
            expected = f"git+{fork['git']}?rev={fork['rev']}#{fork['rev']}"
            for package in packages:
                if package["name"] in fork["packages"]:
                    assert package.get("source") == expected, (lockfile, package)
    assert not (root / "vendor").exists(), "Engine sources belong in the organization forks"
    print("Verified two immutable glory-rs Git forks, six engine/layout packages, one shared type identity per package; ten Glory workspace crates.")


if __name__ == "__main__":
    main()
