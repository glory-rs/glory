# Releases

The coordinated 0.8.0 release requires the exact Stylo, Takia, Wingy and Apace
registry versions recorded in docs/engine-forks.json before Glory publication.
Library, example and benchmark locks remain local and ignored; generate and
validate them without adding them to Git.


Push an annotated `v<version>` tag after updating manifests, local resolutions and reviewed `releases/<version>.md` notes.
The release workflow runs the repository checks before publishing the dependency-ordered `release.json` allowlist to crates.io and creating a GitHub Release.
The required checks include dependency auditing, Rust 1.95 compatibility,
portable Linux tests, Windows host acceptance and actual Chromium suites.
The optional Apple runner and device/reader acceptance are recorded separately.
Configure the repository Actions secret `CARGO_REGISTRY_TOKEN`. Never commit credentials.
Run `python scripts/release.py` with the approved Cargo artifact directory to validate the plan locally.
Builds inherit the caller environment and normal project configuration. Do not redirect Cargo home, target or TEMP/TMP to a global cache root. Obtain artifact paths from `cargo metadata --no-deps --format-version 1` in the owning workspace; do not use or recreate the retired `D:/BuildCache` directory.

If publication tooling fails after all product checks passed, use the manual
**Resume verified release** workflow with the immutable tag and original Release
run ID. It checks that run's repository, workflow, tag, commit and every required
successful product job before any registry operation. Tooling comes from the
workflow revision while every package and release note comes from the frozen tag;
existing registry archives must retain that same commit and checksum.

Cargo resolves dev dependencies while packaging. Unpublished owned dev-only
edges may use command-local Cargo patches to the same frozen source, without
changing manifests, normal/build dependencies or consumer configuration. The
publisher still builds each package and finishes with the registry-only consumer.
The remote immutable tag is rechecked before each upload, including rate-limit retries.
