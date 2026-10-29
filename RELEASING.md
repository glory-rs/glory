# Releases

Push an annotated `v<version>` tag after updating manifests, lockfiles and reviewed `releases/<version>.md` notes.
The release workflow runs the repository checks before publishing the dependency-ordered `release.json` allowlist to crates.io and creating a GitHub Release.
Configure the repository Actions secret `CARGO_REGISTRY_TOKEN`. Never commit credentials.
Run `python scripts/release.py` with the approved Cargo artifact directory to validate the plan locally.
Windows Cargo homes, build output and compiler temporaries belong under `D:/BuildCache`.
