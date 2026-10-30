# Unpublished development versions

This development cycle bumps Glory and Sapid to 0.6.1, Apace to 0.3.0,
Takia to 0.15.1 and Wingy to 0.31.1 without publishing or creating release tags.
Use the five sibling source repositories. Exact dependency requirements prevent
silently mixing these changes with older published packages.

All packages require Rust 1.95 and workspaces use resolver 3. Cargo.lock files
in library, example and benchmark workspaces are ignored; the separate website
repository retains its lockfiles. Check scripts generate a missing local lock once
and then use --locked. To explicitly refresh an existing resolution, run cargo update.

Dependencies were checked against crates.io on 2026-10-30 and updated to the
latest versions compatible with Rust 1.95 and their shared public types.
Compatibility exceptions: Stylo remains at Git revision
78734bf554bb40af9dc11c0eedbe724f6055697c; Boa 0.22 requires ICU 2.3;
anyrender_svg 0.15 requires usvg 0.48.1; Parley/Vello share skrifa 0.44;
rusqlite_migration 2.6 requires rusqlite 0.40. Cap-std/cap-fs-ext 3.4.6,
and rfd 0.16.1 use the available compatible registry versions. Historical benchmark
comparison dependencies remain on the versions they measure.

Normal development CI checks sibling main branches. Frozen publication checks
remain separate and require a future reviewed, publishable dependency graph;
the current source graph is not presented as published release evidence.
See [licensing](licensing.md) for Apache-2.0 and retained third-party notices.
