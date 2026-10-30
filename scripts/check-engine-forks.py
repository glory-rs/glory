"""Verify exact engine versions and source identity in the declared source mode."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib
from library_lockfiles import ensure_library_lockfiles


def run(*args, **kwargs):
    kwargs.setdefault("encoding", "utf-8")
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
    development = fork.get("path") is not None
    if development:
        assert directory.is_dir(), directory
    elif not directory.exists():
        run("git", "clone", "--filter=blob:none", "--no-checkout", fork["git"], str(directory))
    elif not development:
        assert not output("git", "-C", str(directory), "status", "--porcelain"), directory
    assert output("git", "-C", str(directory), "remote", "get-url", "origin").rstrip("/").removesuffix(".git") == fork["git"].rstrip("/").removesuffix(".git")
    if not development:
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
    if not development:
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
    ensure_library_lockfiles(root)
    record = json.loads((root / "docs/engine-forks.json").read_text(encoding="utf-8"))
    metadata = json.loads(output("cargo", "metadata", "--locked", "--all-features", "--format-version", "1", "--manifest-path", str(root / "Cargo.toml")))
    assert Path(metadata["target_directory"]).resolve() == paths["CARGO_TARGET_DIR"]
    development = record.get("source_mode") == "sibling-development"
    members = set(metadata["workspace_members"])
    member_names = {package["name"] for package in metadata["packages"] if package["id"] in members}
    assert member_names == {
        "glory", "glory-style", "glory-render", "glory-core", "glory-resource",
        "glory-i18n", "glory-routing", "glory-ui", "glory-html", "glory-web", "glory-ssr", "glory-apace",
        "glory-salvo", "glory-view-macros",
    }, member_names
    compiler = next(package for package in metadata["packages"] if package["name"] == "glory-view-macros")
    assert any("proc-macro" in target["kind"] for target in compiler["targets"])
    for features in ([], ["--features", "web"]):
        graph = output("cargo", "tree", "--locked", "-p", "glory", "--no-default-features", *features,
                       "--edges", "normal,build", "--prefix", "none", "--manifest-path", str(root / "Cargo.toml"))
        assert "glory-view-macros" not in graph, "runtime graph acquired the optional display compiler"
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    for fork in record["forks"]:
        assert len(fork["rev"]) == 40 and all(c in "0123456789abcdef" for c in fork["rev"])
        if development:
            directory = (root / fork["path"]).resolve()
            assert Path(output("git", "-C", str(directory), "rev-parse", "--show-toplevel")).resolve() == directory
            run("git", "-C", str(directory), "merge-base", "--is-ancestor", fork["rev"], "HEAD")
        source = None if development else "registry+https://github.com/rust-lang/crates.io-index"
        for name in fork["packages"]:
            version = fork.get("versions", {}).get(name, fork["version"])
            resolved = [package for package in metadata["packages"] if package["name"] == name]
            assert len(resolved) == 1 and resolved[0]["source"] == source, (name, resolved)
            assert resolved[0]["version"] == version, (name, resolved[0]["version"], version)
            locked = [package for package in lock["package"] if package["name"] == name]
            assert len(locked) == 1 and locked[0].get("source") == source, (name, locked)
            assert locked[0]["version"] == version, (name, locked)
            if development:
                expected_root = (root / fork["path"]).resolve()
                assert Path(resolved[0]["manifest_path"]).resolve().is_relative_to(expected_root), (name, resolved)
                continue
            assert len(locked[0].get("checksum", "")) == 64, (name, locked)
            archive_root = Path(resolved[0]["manifest_path"]).parent
            vcs = json.loads((archive_root / ".cargo_vcs_info.json").read_text(encoding="utf-8"))
            expected_revision = fork.get("revisions", {}).get(name, fork["rev"])
            assert vcs["git"]["sha1"] == expected_revision, (name, vcs, expected_revision)
            archive = paths["CARGO_HOME"] / "registry/cache" / archive_root.parent.name / f"{name}-{version}.crate"
            assert hashlib.sha256(archive.read_bytes()).hexdigest() == locked[0]["checksum"], name
        if options.test:
            directory = (root / fork["path"]).resolve() if development else paths["CARGO_TARGET_DIR"].parent / "engine-forks" / fork["name"]
            directory.parent.mkdir(parents=True, exist_ok=True)
            test_fork(fork, directory, paths["CARGO_TARGET_DIR"], options.android)
    retired = {"glory-apace-dom", "glory-apace-traits", "glory-apace-paint", "glory-apace-shell", "glory-takia", "glory-stylo-takia"}
    for lockfile in (
        root / "Cargo.lock",
        root / "examples/semantic-web/Cargo.lock",
        root / "examples/counter/Cargo.lock",
        root / "examples/mobile/Cargo.lock",
    ):
        packages = tomllib.loads(lockfile.read_text(encoding="utf-8"))["package"]
        assert not retired.intersection(package["name"] for package in packages), lockfile
        for fork in record["forks"]:
            expected = None if development else "registry+https://github.com/rust-lang/crates.io-index"
            for package in packages:
                if package["name"] in fork["packages"]:
                    assert package.get("source") == expected, (lockfile, package)
                    assert package["version"] == fork.get("versions", {}).get(package["name"], fork["version"]), (lockfile, package)
        legacy = {"blitz-dom", "blitz-traits", "blitz-paint", "blitz-shell", "stylo_taffy", "taffy", "winit", "winit-core"}
        assert not legacy.intersection(package["name"] for package in packages), lockfile
        assert not any(package["name"].startswith(("winit-", "glory-blitz", "blitz-")) for package in packages), lockfile
    assert not (root / "vendor").exists(), "Engine sources belong in the organization forks"
    print(f"Verified {len(record['forks'])} engine families ({record.get('source_mode', 'published-registry')}) with exact versions and source identity; {sum(len(fork['packages']) for fork in record['forks'])} engine/platform packages with one shared type identity; {len(member_names)} Glory workspace crates.")


if __name__ == "__main__":
    main()
