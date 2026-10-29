#!/usr/bin/env python3
"""Validate and publish this repository's explicit, dependency-ordered release."""
import argparse
from email.utils import parsedate_to_datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import time
import tomllib
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def run(*args, capture=False):
    result = subprocess.run(args, cwd=ROOT, check=True, text=True, encoding="utf-8",
                            stdout=subprocess.PIPE if capture else None)
    return result.stdout.strip() if capture else None


def publish(command):
    """Retry only an explicit registry rate limit, at the server's stated time."""
    for attempt in range(12):
        result = subprocess.run(command, cwd=ROOT, text=True, encoding="utf-8", errors="replace",
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        print(result.stdout, end="", flush=True)
        if result.returncode == 0:
            return
        retry = re.search(r"try again after (.+? GMT)", result.stdout)
        if "429 Too Many Requests" not in result.stdout or not retry:
            raise subprocess.CalledProcessError(result.returncode, command)
        deadline = parsedate_to_datetime(retry[1])
        # Prefer registry time: runner clocks may differ from the registry clock.
        request = urllib.request.Request("https://crates.io/api/v1/crates/takia/0.15.0",
                                         headers={"User-Agent": "glory-release/1.0"})
        with urllib.request.urlopen(request, timeout=30) as response:
            server_now = parsedate_to_datetime(response.headers["Date"])
        delay = max(5, (deadline - server_now).total_seconds() + 5)
        if delay > 3600:
            raise SystemExit("Registry asked for a delay longer than one hour; resume this release later")
        print(f"Registry rate limit: retry after {deadline.isoformat()} (about {delay:.0f}s).", flush=True)
        end = time.monotonic() + delay
        while time.monotonic() < end:
            time.sleep(min(30, max(0, end - time.monotonic())))
    raise SystemExit("Repeated registry rate limits; resume this release later")


def validate_archive(archive, expected_commit):
    """Inspect the actual upload, including dereferenced license/readme contents."""
    with tarfile.open(archive) as source:
        records = source.getmembers()
        provenance = next((entry for entry in records if entry.name.endswith("/.cargo_vcs_info.json")), None)
        if provenance is None:
            raise SystemExit("Packaged source has no Git provenance")
        vcs = json.load(source.extractfile(provenance))
        if vcs["git"]["sha1"] != expected_commit or vcs["git"].get("dirty"):
            raise SystemExit("Packaged source differs from the reviewed release commit")
        for entry in records:
            basename = entry.name.rsplit("/", 1)[-1].upper()
            if not basename.startswith(("LICENSE", "COPYING", "README")) or entry.isdir():
                continue
            try:
                content = source.extractfile(entry).read().strip()
            except (KeyError, OSError, AttributeError) as failure:
                raise SystemExit(f"Archive contains an unresolved link: {entry.name}") from failure
            if re.fullmatch(rb"\.{1,2}[/\\][^\r\n]+", content):
                raise SystemExit(f"Archive contains an unresolved link instead of file contents: {entry.name}")
    return vcs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--publish", action="store_true")
    parser.add_argument("--tag")
    parser.add_argument("--check-package", help="package and inspect one owned crate without uploading")
    options = parser.parse_args()
    plan = json.loads((ROOT / "release.json").read_text(encoding="utf-8"))
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    workspace = manifest.get("workspace", {})
    version = workspace.get("package", {}).get("version") or manifest["package"]["version"]
    expected_tag = plan.get("tag_prefix", "v") + version
    event_tag = os.environ.get("GITHUB_REF_NAME") if os.environ.get("GITHUB_REF_TYPE") == "tag" else None
    tag = options.tag or event_tag or expected_tag
    if tag != expected_tag:
        raise SystemExit(f"Tag {tag!r} must match manifest version: {expected_tag}")
    metadata = json.loads(run("cargo", "metadata", "--no-deps", "--format-version", "1", capture=True))
    target = Path(metadata["target_directory"]).resolve()
    configured = os.environ.get("CARGO_TARGET_DIR")
    if not configured or target != Path(configured).resolve():
        raise SystemExit("Set and verify CARGO_TARGET_DIR before releasing")
    if os.name == "nt" and not str(target).replace("\\", "/").lower().startswith("d:/buildcache/"):
        raise SystemExit("Windows release artifacts must stay under D:/BuildCache")
    packages = {p["name"]: p for p in metadata["packages"]}
    for relative in plan.get("extra_manifests", []):
        extra = json.loads(run("cargo", "metadata", "--manifest-path", str(ROOT / relative),
                               "--no-deps", "--format-version", "1", capture=True))
        if Path(extra["target_directory"]).resolve() != target:
            raise SystemExit(f"Extra workspace has a different artifact directory: {relative}")
        for package in extra["packages"]:
            if package["name"] in packages:
                raise SystemExit(f"Duplicate package in extra workspace: {package['name']}")
            packages[package["name"]] = package
    selected = [packages[name] for name in plan["packages"]]
    owned = {p["name"]: p for p in selected}
    for name, commit in plan.get("reused_package_commits", {}).items():
        if name not in owned or not re.fullmatch(r"[0-9a-f]{40}", commit):
            raise SystemExit("Reused packages require an owned name and an explicit source commit")
    completed = set()
    for package in selected:
        if plan.get("uniform_version") and package["version"] != version:
            raise SystemExit(f"Package version differs from the release: {package['name']}")
        if package.get("publish") == []:
            raise SystemExit(f"{package['name']} disables publication")
        if package.get("repository") != "https://github.com/" + plan["repository"]:
            raise SystemExit(f"Wrong repository metadata: {package['name']}")
        if not any(package["name"] == prefix or package["name"].startswith(prefix + "-")
                   or package["name"].startswith(prefix + "_") for prefix in plan["allowed_prefixes"]):
            raise SystemExit(f"Refusing to publish outside owned namespaces: {package['name']}")
        for dependency in package["dependencies"]:
            if dependency["kind"] == "dev":
                continue
            if dependency.get("source", "") and dependency["source"].startswith("git+"):
                raise SystemExit(f"Git dependency would change when published: {package['name']} -> {dependency['name']}")
            if dependency["name"] in owned and dependency["name"] not in completed:
                raise SystemExit(f"Release order must place {dependency['name']} before {package['name']}")
            if dependency.get("path") and dependency["name"] not in owned:
                external = plan.get("external_path_dependencies", {}).get(dependency["name"])
                if external != dependency["req"] or not external.startswith("="):
                    raise SystemExit(f"Unreleased path dependency: {package['name']} -> {dependency['name']}")
        completed.add(package["name"])
    print(json.dumps({"tag": tag, "repository": plan["repository"],
                      "packages": [{"name": p["name"], "version": p["version"]} for p in selected]}, indent=2), flush=True)
    if options.check_package:
        if options.publish or options.check_package not in owned:
            raise SystemExit("Archive checks require an owned package and cannot publish")
        package = owned[options.check_package]
        run("cargo", "package", "--locked", "--no-verify", "--manifest-path", package["manifest_path"], "-p", package["name"])
        validate_archive(target / "package" / f"{package['name']}-{package['version']}.crate",
                         run("git", "rev-parse", "HEAD", capture=True))
        return
    if not options.publish:
        return
    notes = ROOT / "releases" / (version + ".md")
    if not notes.is_file():
        raise SystemExit("A reviewed release-notes file is required")
    if run("git", "status", "--porcelain", capture=True):
        raise SystemExit("Publication requires a clean checkout")
    head = run("git", "rev-parse", "HEAD", capture=True)
    if run("git", "rev-parse", tag + "^{commit}", capture=True) != head:
        raise SystemExit("Publication must run at the exact release tag")
    if not os.environ.get("CARGO_REGISTRY_TOKEN"):
        raise SystemExit("CARGO_REGISTRY_TOKEN is required")
    cross = set(plan.get("cross_target_packages", []))
    if cross:
        # Cross-target packages cannot be verified by the publisher's native
        # linker. Require the successful validation jobs from this same run.
        if not os.environ.get("GITHUB_ACTIONS") or not os.environ.get("GITHUB_RUN_ID"):
            raise SystemExit("Cross-target publication requires the gated Actions release workflow")
        jobs = json.loads(run("gh", "api", f"repos/{plan['repository']}/actions/runs/{os.environ['GITHUB_RUN_ID']}/jobs?per_page=100", capture=True))
        succeeded = {job["name"] for job in jobs["jobs"] if job["conclusion"] == "success"}
        if not set(plan.get("required_release_jobs", [])).issubset(succeeded):
            raise SystemExit("Required platform validation jobs have not all succeeded")
    artifacts = []
    release_packages = []
    output = target / "release" / tag
    output.mkdir(parents=True, exist_ok=True)
    for package in selected:
        name, package_version = package["name"], package["version"]
        expected_commit = plan.get("reused_package_commits", {}).get(name, head)
        request = urllib.request.Request(f"https://crates.io/api/v1/crates/{name}/{package_version}",
                    headers={"User-Agent": "glory-release/1.0 (https://github.com/" + plan["repository"] + ")"})
        existing = None
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                existing = json.load(response)["version"]
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
        if existing:
            if existing.get("repository") != "https://github.com/" + plan["repository"]:
                raise SystemExit(f"Existing {name} {package_version} belongs to another source")
            print(f"Already published: {name} {package_version}", flush=True)
        else:
            if expected_commit != head:
                raise SystemExit(f"A reused package must already exist in the registry: {name}")
            # Packaging is reversible. Inspect its exact contents before the
            # immutable registry upload, including Windows Git symlink failures.
            run("cargo", "package", "--locked", "--no-verify", "--manifest-path", package["manifest_path"], "-p", name)
            validate_archive(target / "package" / f"{name}-{package_version}.crate", head)
            command = ["cargo", "publish", "--locked", "--manifest-path", package["manifest_path"], "-p", name]
            if name in cross:
                command.append("--no-verify")
            publish(command)
        # Let sparse-index propagation finish before building dependent crates.
        ready = False
        for attempt in range(30):
            try:
                with urllib.request.urlopen(request, timeout=30) as response:
                    published = json.load(response)["version"]
                ready = True
                break
            except urllib.error.HTTPError as error:
                if error.code != 404:
                    raise
                time.sleep(2)
        if not ready:
            raise SystemExit(f"Published version did not become visible: {name} {package_version}")
        archive = output / f"{name}-{package_version}.crate"
        download = urllib.request.Request(f"https://static.crates.io/crates/{name}/{name}-{package_version}.crate",
                                           headers={"User-Agent": "glory-release/1.0"})
        with urllib.request.urlopen(download, timeout=60) as response:
            archive.write_bytes(response.read())
        checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
        if checksum != published["checksum"]:
            raise SystemExit(f"Registry checksum differs from packaged artifact: {name}")
        validate_archive(archive, expected_commit)
        artifacts.append(archive)
        release_packages.append({"name": name, "version": package_version,
                                 "checksum": checksum, "source_commit": expected_commit})
    if plan.get("registry_verifier"):
        run("python", str(ROOT / plan["registry_verifier"]), "--tag", tag)
        artifacts.extend([output / "registry-consumer.json", output / "registry-consumer.lock"])
    evidence = output / "release.json"
    evidence.write_text(json.dumps({"repository": plan["repository"], "tag": tag, "commit": head,
        "packages": release_packages}, indent=2) + "\n", encoding="utf-8")
    exists = subprocess.run(["gh", "release", "view", tag, "--repo", plan["repository"]],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    if not exists:
        command = ["gh", "release", "create", tag, "--repo", plan["repository"], "--verify-tag",
                   "--title", plan["title"] + " " + version, "--notes-file", str(notes)]
        if "-" in version:
            command.append("--prerelease")
        run(*command, str(evidence), *map(str, artifacts))


if __name__ == "__main__":
    main()
