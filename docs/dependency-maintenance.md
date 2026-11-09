# Dependency maintenance

Dependabot opens daily Cargo version-update PRs for the independent `tools/cli`
workspace and weekly updates for GitHub Actions. The CLI uses registry dependencies
only and does not depend on Glory runtime or sibling engine workspaces.

The runtime, maintained examples and Glory benchmarks intentionally resolve the
coordinated Apace, Takia and Wingy sibling checkouts documented in
[`engine-forks.json`](engine-forks.json). GitHub-hosted Dependabot checks out only
this repository, so it cannot retrieve those out-of-repository path dependencies.
The previous combined Cargo update job failed during file fetching, before it
could propose any updates. Splitting the directories does not supply the missing
engine manifests. Excluding engine manifests also leaves an incomplete Cargo
graph; it cannot preserve a working update job for that graph.

Runtime version updates therefore require a coordinated maintainer PR with the
declared sibling checkouts available. Update the relevant manifest constraints,
run `python scripts/library_lockfiles.py` to prepare locks for the explicit
maintained-workspace list, and run the semantic/quality gates before merging.
Engine source/version changes must also update and verify `docs/engine-forks.json`
with `python scripts/check-engine-forks.py --test`. Do not replace development
paths with registry dependencies just to satisfy the bot, or add consumer
repositories to upstream checks.

Security auditing remains enabled for the runtime and every maintained workspace:
`quality.yml` checks out the sibling engines, resolves all eleven workspaces in
`scripts/library_lockfiles.py`, and runs `scripts/check-dependencies.py` with pinned
`cargo-audit`. This gate runs through semantic CI on PRs/main pushes and on the
weekly quality schedule. It uploads the generated locks and audit evidence.
Narrowing Dependabot version-update PRs does not exclude any of those workspaces
or advisories from that audit. Runtime automatic version-update PRs are unavailable
under the current sibling-source arrangement; security findings still require
maintainer fixes through the same CI gates.
