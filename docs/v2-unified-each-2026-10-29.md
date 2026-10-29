# Unified Each

`glory::ui::Each` and `glory::display::Each` now export the same type and
controller. Both handwritten builders and the DSL accept owned `Vec<T>`,
`Cage<Vec<T>>`, `Bond<Vec<T>>`, `Vary<Vec<T>>` and `KeyedList<K,T>` directly.
Constructing the builder does not read the source or run row factories.

```rust,ignore
// A vector needs an explicit, unique, stable key.
let vector_widget = glory::ui::each(values)
    .key(|value| value.id)
    .children(|row| {
        let label = row.field("label", |value| &value.label, |value| &mut value.label);
        glory::view! { <span>{label}{row.index}</span> }
    });

// KeyedList already owns keys and membership identities.
let member_widget = glory::ui::each(members).children(|row| {
    let label = row.field("label", |value| &value.label, |value| &mut value.label);
    glory::view! { <span>{label}</span> }
});
```

The equivalent DSL is:

```rust,ignore
view! {
    <div>
        <Each each={values} key={|value| value.id} children={|row| {
            let label = row.field("label", |value| &value.label, |value| &mut value.label);
            view! { <span>{label}{row.index}</span> }
        }}/>
        <Each each={members} children={|row| {
            let label = row.field("label", |value| &value.label, |value| &mut value.label);
            view! { <span>{label}</span> }
        }}/>
    </div>
}
```

`each(...).children(...)` is an ordinary `Widget`. It can be mounted on the
standard browser, SSR, stream and native hosts or filled into another Widget.
The sparse handwritten API uses
`display::view(&TEMPLATE).child(BODY, each(members).children(factory))`;
the DSL uses this same content installation path.

## Row and source semantics

Both row handles expose `key()`, `field(...)`, `update(...)` and
`try_with_untracked(...)`. Their representation follows the authoritative source:

| Source | Row handle and updates |
|---|---|
| Vector / Cage / Bond / Vary | `ValueRow<K,T>` contains reactive `value: Cage<T>` and `index: Cage<usize>`; retained keys receive new values and positions |
| KeyedList | Existing `KeyedEntry<K,T>` directly projects member fields; `T` need not implement Clone; no item/index mirror or data Owner is added |

This distinction does not add another Each API. It preserves the source's update
model and avoids converting KeyedList into a cloned vector or rebuilding a
KeyedList from every vector revision. Full vector observation still clones the
vector as before; use KeyedList for independently edited member fields. A keyed
field edit does not notify order observers or rerun row factories.

Vector row handles retain the previous `ui::Each` snapshot semantics: writes to
`row.value`, its projected fields or `row.update` change the local row value;
write the original vector to publish an authoritative collection change.
KeyedEntry edits write the authoritative member directly. Keeping this explicit
preserves existing controlled-widget behavior and avoids silently introducing
bidirectional synchronization between a vector and its row snapshots.

Factories run once for a live key/membership. Moving a row retains its Widget,
physical nodes, handlers and local state. Removing it retires its Owner, aliases
and captures. A later insertion gets a fresh row. KeyedList generations also
distinguish removal/reinsertion within one observation interval. A vector's same
key replacement in one observation is an update to its existing row.

Vectors must supply `.key(...)` / `key={...}`; index identity is never inferred.
Duplicate keys are rejected by the existing Cache before row mutations. Foreign
and disposed sources are rejected before mounting. Disposal after mounting reports
a typed access error through the host, rather than panicking on an order read.

## Initial-value rows

For rows that intentionally ignore later item/index changes, use
`each(values).key(key).seed(factory)` or `<Each each={values} key={key}
seed={factory}/>`; KeyedList supports the same `.seed` mode for Clone values.
The factory receives the initial `T`, keeps its row-local state on movement and
does not allocate item/index cages or an extra data Owner. `children` and `seed`
are mutually exclusive. The former `each_seed` function is retired.

## Implementation and verification

One monomorphized controller in `crates/ui/src/each` owns source observation,
row construction/update/retirement and the unchanged keyed/LIS `list::Cache`.
Small physical mount adapters handle semantic ranges and sparse browser ranges.
Compile-time ownership policies keep KeyedList and seed rows free of optional
vector-owner storage. No per-HTML-element semantic records or second data model
are introduced.

The shared fixtures exercise both sources on sparse browser mounts, mixed-tree
CSR/SSR hydration, live incremental streams and native layout/input/painting.
They check node identity, dirty input/selection through movement and adoption,
reactive vector indices, state reset on reinsertion, direct non-Clone member
fields, bounded ownership and cleanup. Existing density, failure-prefix and GC
checks remain applicable. See the [verification and measured results](v2-unified-each-results-2026-10-29.md)
for frozen inputs, correctness evidence, timing differences and their limits;
syntax and allocation counts alone are not a timing result.
