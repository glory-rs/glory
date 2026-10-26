"""Verify immutable engine Git dependencies; optionally test clean fork checkouts."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
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
        assert portable.startswith(("d:/buildcache/", "/mnt/d/buildcache/")) or (sys.platform == "darwin" and value.is_relative_to("/Volumes/Data/Caches/glory")), (name, value)
        paths[name] = value
    return paths


def test_fork(fork, directory, target, android):
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
    for index, source in enumerate(fork.get("rustc_tests", [])):
        executable = target / f"{fork['name']}-standalone-{index}{'.exe' if os.name == 'nt' else ''}"
        run("rustc", "--edition", "2024", "--test", str(directory / source), "-o", str(executable))
        run(str(executable))
    if sys.platform == "darwin":
        for arguments in fork.get("apple_checks", []):
            run("cargo", "check", "--locked", "--manifest-path", manifest, *arguments)
    if android:
        for arguments in fork.get("android_checks", []):
            run("cargo", "check", "--locked", "--manifest-path", manifest, *arguments)
    assert not output("git", "-C", str(directory), "status", "--porcelain"), directory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test", action="store_true", help="also run engine unit tests in separate Git checkouts under the D: build root")
    parser.add_argument("--android", action="store_true", help="with --test, also check native/game Activity targets using an explicitly installed NDK")
    options = parser.parse_args()
    if options.android and not options.test:
        parser.error("--android requires --test")
    paths = build_environment()
    root = Path(__file__).resolve().parents[1]
    record = json.loads((root / "docs/engine-forks.json").read_text(encoding="utf-8"))
    metadata = json.loads(output("cargo", "metadata", "--locked", "--all-features", "--format-version", "1", "--manifest-path", str(root / "Cargo.toml")))
    assert Path(metadata["target_directory"]).resolve() == paths["CARGO_TARGET_DIR"]
    members = set(metadata["workspace_members"])
    member_names = {package["name"] for package in metadata["packages"] if package["id"] in members}
    assert member_names == {
        "glory", "glory-style", "glory-render", "glory-core", "glory-resource",
        "glory-i18n", "glory-ui", "glory-html", "glory-web", "glory-ssr", "glory-blitz",
        "glory-salvo",
    }, member_names
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
            test_fork(fork, directory, paths["CARGO_TARGET_DIR"], options.android)
    retired = {"glory-blitz-dom", "glory-blitz-traits", "glory-blitz-paint", "glory-blitz-shell", "glory-taffy", "glory-stylo-taffy"}
    for lockfile in (
        root / "Cargo.lock",
        root / "examples/semantic-web/Cargo.lock",
        root / "examples/counter/Cargo.lock",
        root / "examples/mobile/Cargo.lock",
    ):
        packages = tomllib.loads(lockfile.read_text(encoding="utf-8"))["package"]
        assert not retired.intersection(package["name"] for package in packages), lockfile
        for fork in record["forks"]:
            expected = f"git+{fork['git']}?rev={fork['rev']}#{fork['rev']}"
            for package in packages:
                if package["name"] in fork["packages"]:
                    assert package.get("source") == expected, (lockfile, package)
    assert not (root / "vendor").exists(), "Engine sources belong in the organization forks"
    print(f"Verified {len(record['forks'])} immutable glory-rs Git forks, {sum(len(fork['packages']) for fork in record['forks'])} engine/platform packages, one shared type identity per package; {len(member_names)} Glory workspace crates.")


if __name__ == "__main__":
    main()
