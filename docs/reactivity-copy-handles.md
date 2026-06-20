# Reactivity Copy Handle Decision

Date: 2026-06-12

This note closes the R5 evaluation from `_todos.md`: whether Glory should move
its reactive handles to a Dioxus-style generational-box model so handles are
`Copy`.

## Current State

- `Cage<T>` is already `Copy`.
- A `Cage<T>` points at a leaked slot with an identity and generation token.
- `Owner` invalidation drops the stored value, clears subscriptions, bumps the
  generation, and parks the slot in a type-keyed free list for reuse.
- Stale `Copy` handles cannot access a recycled slot because their generation
  no longer matches.
- `Bond<T>` is `Clone`, not `Copy`. It owns shared `Rc` state for the mapper,
  cached value, dependency set, dependency version snapshot, subscribers, and
  optional equality gate.

## Options Considered

Keep the current split:

- `Cage<T>` stays `Copy` for event handlers and row state.
- `Bond<T>` stays `Clone`, where cloning is an `Rc` bump for several shared
  fields.

Move `Bond<T>` to a generational slot:

- Store the mapper, cached value, dependency snapshot, subscribers, and equality
  gate in a slot.
- Make the public `Bond<T>` handle store only slot identity plus generation.
- Reuse the same stale-handle checks that `Cage<T>` uses.

Move all reactive handles to a shared untyped arena:

- Use one generational arena for cages and bonds.
- Type erase values and mappers behind dynamic storage.
- Add typed accessors on top.

## Decision

Do not migrate further right now.

`Cage<T>` already provides the main ergonomic win: event handlers and row widgets
can copy mutable state handles without `clone()` noise. `Bond<T>` clone cost is
small in the current architecture, and making it `Copy` would move much more
complex state into a lifetime-managed slot. That would add risk around:

- dependency set replacement when a mapper starts tracking different sources;
- equality-gated versions and stale cached values;
- subscriber cleanup for derived values used across widget lifetimes;
- dynamic mapper storage, which is harder to make observable in devtools;
- preserving the existing `Bond<T>` API without exposing arena failure modes.

The current `Cage<T>` slot/free-list design also avoids the original memory
problem: invalidated cages drop their stored value and reuse slots, so live slot
count is bounded by peak concurrent cages instead of total cages ever created.

## Revisit Triggers

Reopen this decision only if measurement shows reactive handle cloning is a
material cost. Concrete triggers:

- E10 official benchmark summaries show a repeatable regression attributable to
  `Bond<T>` cloning or allocation.
- A focused Criterion benchmark shows `Bond<T>::clone` or derived-handle
  allocation consumes at least 1% of a hot render/update path.
- A large app reports derived-handle churn that remains after normal ownership
  cleanup and static-subtree work.

If one of those happens, prototype a `BondSlot<T>` behind the existing API first.
Do not move cages and bonds into a shared untyped arena until a typed bond slot
has proven worthwhile.
