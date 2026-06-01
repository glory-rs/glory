# Hot Reload Follow-up Tasks

This file splits IMP-008 into concrete implementation tasks.

- [x] Add a public `reloadable_view!` marker matching Glory's builder-pattern view surface.
- [x] Stop `glory-cli watch --hot-reload` from wiring the unsupported `ViewMacros` / `view!` patch path.
- [x] Keep the active hot-reload transport focused on `HotFunctions` function replacement events.
- [x] Update hot-reload documentation to describe the builder-style surface and legacy virtual-node internals accurately.
- [x] Add unit coverage for `reloadable_view!` registration and existing function replacement scanning.
