# M5 Hot Reload Tasks

Aggressive execution checklist for `_todos.md` §5 P2.

## Batch 1: Runtime Closure Relink

- [x] Add a type-safe reloadable function handle for builder-style closures.
- [x] Add a registry keyed by stable function id.
- [x] Support replacing a function body while existing handles keep calling the
      newest body.
- [x] Add tests for register / replace / type mismatch.

## Batch 2: CLI Integration

- [x] Teach `glory-cli watch --hot-reload` to emit function replacement events
      in addition to macro patch payloads.
- [x] Add a small builder-style example that registers a reloadable view
      function.

## Batch 3: State Preservation

- [x] Tie reloadable function ids to the M2 Owner model so stateful closures keep
      their `Cage` handles across relinks.
