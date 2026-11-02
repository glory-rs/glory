# Upstream build boundary

Glory's runtime, tooling, tests, examples, benchmarks and default CI must build
without a downstream component repository. Consumer compatibility suites belong
in the consumer repository. The declared rendering engines in
`docs/engine-forks.json` use sibling sources for Apace, Takia and Wingy during
coordinated development, and checksum-verified registry packages for Stylo.

`scripts/library_lockfiles.py` prepares only the explicit `LIBRARY_MANIFESTS`
workspaces. It does not discover nested Cargo projects or resolve peer-framework
comparison fixtures. Jobs can select their own workspace with repeatable
`--manifest-path` arguments; dependency auditing uses the same maintained list.

```sh
python scripts/library_lockfiles.py --manifest-path Cargo.toml
python scripts/library_lockfiles.py --manifest-path examples/counter/Cargo.toml
python -m unittest discover -s scripts -p test_library_lockfiles.py
```

The display examples exercise raw HTML, custom components and Glory's own
semantic controls, including SSR, hydration and streaming. Downstream-specific
widgets and feature-size scenarios are not imported by these fixtures. The
current size matrix contains only Glory scenarios; previous JSON measurements
retain their original provenance and are not dependency declarations.

Boundary regressions check that unlisted consumer workspaces are never resolved,
explicit scopes do not touch unrelated workspaces, target validation fails before
resolution, and repository path dependencies stay inside Glory or its declared
engines. To verify a clean checkout, copy the tracked sources into a fresh parent
directory with only the declared engine siblings, then run the default lockfile
preparation. No consumer checkout or preexisting lockfile is required.
