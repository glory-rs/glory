# M2 Reactivity Modernization Tasks

This is the aggressive execution checklist for `_todos.md` §2 P0. The goal is
to make progress in small, reviewable steps even while the full owner-aware
generational arena remains large.

## Batch 1: Copy Cage Handle

- [x] Replace `Cage<T>`'s `Rc<Cell<_>>` / `Rc<RefCell<_>>` fields with a small
      copyable handle to leaked per-cage state.
- [x] Implement `Copy` for `Cage<T>` while keeping existing `.clone()` callsites
      source-compatible.
- [x] Add `Cage::try_get`, `Cage::try_get_untracked`, `Cage::try_revise`, and
      `Cage::try_revise_silent` as fallible borrow APIs.
- [x] Preserve current subscription, serialization, equality, and scheduler
      semantics under the new storage layout.
- [x] Update `_todos.md` to record the completed sub-slice and the later
      owner / sync-storage completion.
- [x] Run targeted tests for `glory-core` with `web-ssr`.

## Batch 2: Owner And Reclamation

- [x] Introduce an `Owner` type that records allocated cage slots for a `Scope`.
- [x] Thread the owner through root and child scopes.
- [x] On scope drop, invalidate owned handles and bump generations.
- [x] Convert stale-handle APIs from panic-only to `Result` where caller recovery
      is practical.

## Batch 3: Sync Storage

- [x] Add a `sync-storage` feature.
- [x] Implement `SyncStorage` over `RwLock` / atomics.
- [x] Decide whether `Cage<T>` switches storage backends by feature or whether a
      separate sync handle is exposed first.
- [x] Add thread-crossing tests for sync handles.
