# Releases

The coordinated 0.6.1 release requires the exact Stylo, Takia, Wingy and Apace
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
Windows Cargo homes, build output and compiler temporaries belong under `D:/BuildCache`.
