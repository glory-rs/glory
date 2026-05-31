# M3 Renderer Abstraction Tasks

Aggressive execution checklist for `_todos.md` §3 P0.

## Required Method Table

- [x] Element creation: `create_element(name, is_void)`.
- [x] Attribute mutation: `set_attribute`, `remove_attribute`.
- [x] Property mutation: `set_property`, `remove_property`.
- [x] Class mutation: `add_class`, `remove_class`.
- [x] Text / HTML sinks: `set_text`, `set_html`.
- [x] Child positioning: `insert_child(parent, child, Head | Tail | Before | After)`.
- [x] Child removal: `remove_child(parent, child)`.
- [x] Identity comparison: `node_identity_eq(a, b)`.
- [x] Event bridge: `attach_event(node, name, bubbles, handler)`.

## Batch 1: Trait Skeleton

- [x] Add `crates/core/src/renderer/` with the core `Renderer` trait.
- [x] Add `EventPayload` and erased event payload types.
- [x] Add `SsrRenderer`.
- [x] Add `WebRenderer` behind `wasm32 + web-csr`.
- [x] Add SSR renderer tests for create / attr / property / class / insert /
      remove / identity.
- [x] Export `renderer` from `glory-core`.

## Batch 2: Widget Migration

- [ ] Replace direct SSR element node calls with `SsrRenderer`.
- [ ] Replace direct CSR element node calls with `WebRenderer`.
- [ ] Introduce a generic rendered element type once the two paths share enough
      shape.
- [ ] Move attr / prop injection to renderer value commands.

## Batch 3: MockRenderer

- [ ] Add a pure in-memory `MockRenderer` that records command sequences.
- [ ] Port `Each` reorder assertions from final HTML checks to command sequence
      checks.
- [ ] Use MockRenderer for fast component-level regressions.
