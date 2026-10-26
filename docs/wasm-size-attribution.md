# Minimal WASM: retained code attribution

Analyzed on 2026-10-26 against the optimized `glory-minimal` case in the
[36-case baseline](wasm-size-baseline.md). This application mounts an empty
semantic Group. Final WASM is **894,092 B**, or **282,146 B with Brotli**;
the separately served wasm-bindgen JS is 59,232 B / 8,816 B Brotli including
transitively imported snippets. Application CSS/fonts/images are separate.

## Method

The normal Binaryen output strips function names. This analysis reruns the
same pinned Binaryen `-Oz` transformation with `-g`, then verifies that **every
non-custom section has the same SHA-256** as the measured, stripped artifact.
This confirms that the names describe the final code, rather than only the
larger, pre-Binaryen wasm-bindgen output.

[`minimal-attribution.json`](../benchmarks/wasm-size/minimal-attribution.json)
preserves section hashes, all aggregate counts and the largest functions.
There are 2,221 defined functions containing 796,143 B of function bodies.
The code section also contains vector and body-length encodings.

Each function body is counted once under its enclosing symbol. Inlined callees
are charged to that enclosing function; generic standard-library functions are
charged to their owning type/module. These are **not exclusive crate sizes**,
and are not predictions of the savings from removing a crate or capability.
In particular, much of `alloc`/`core` here implements Glory's collections and
owned values. Compression is not additive per function or crate.

## Where the bytes are

| Final WASM component | Bytes | Share |
|---|---:|---:|
| Code section payload | 799,205 | 89.39% |
| Data section payload | 78,448 | 8.77% |
| Other sections, encodings and module header | 16,439 | 1.84% |
| Total | 894,092 | 100% |

There is no retained `name` or DWARF section. The remaining custom sections
are `producers` and `target_features` (277 B including their headers). Stripping
debug information again cannot explain or remove the near-900 KB floor.

The function bodies split as follows. This is a separate accounting of the
796,143 B of bodies, not another layer to add to the section table.

| Enclosing symbols | Function-body bytes |
|---|---:|
| `glory_web` | 283,202 |
| Rust `alloc` + `core` + `std` | 250,392 |
| `glory_render` | 103,125 |
| `glory_html` | 68,207 |
| `glory_ui` | 21,593 |
| `glory_core` | 21,163 |
| Other symbols / glue / allocator / bindings | 48,461 |

Important retained functions after Binaryen inlining:

| Enclosing function | Bytes | Retained behavior |
|---|---:|---|
| Events listener callback | 51,305 | Shared native-event dispatcher and inlined handlers |
| `launch_with<Group, ui>` | 42,176 | Mount/startup and code inlined into this entry point |
| `Renderer::apply` | 32,898 | General semantic mutations and adapter updates |
| `Mapper::apply<Sink>` | 31,331 | Shared semantic-to-HTML mapping and DOM edits |
| `Renderer::drain` | 29,631 | Host observations, including rich/file/clipboard/motion paths |
| `Renderer::action` | 22,515 | Finite actions for all supported control types |
| `DescriptorView::validate_fields` | 16,541 | Validation for the finite semantic node model |
| `DocumentLayers::sync` | 12,875 | Popover/modal/focus synchronization |

The launch function's entire body is not necessarily startup-only code; inlining
changes where retained instructions are attributed. Likewise, the 11,393 B of
functions named under `browser::rich` is not the whole rich-editor footprint:
other editor code has been inlined into events, actions, drains and validators.

## Why an empty Group retains these paths

The current host is a general interpreter. `Session` stores
`RefCell<Box<dyn Render>>` in [`app.rs`](../crates/ui/src/app.rs). Its renderer
vtable refers to the complete browser `apply`, `drain`, `action`, `request` and
other hooks. The app's initial `Group` does not specialize those hooks to a
Group-only implementation.

[`Renderer::new`](../crates/web/src/browser.rs) constructs the shared listeners.
[`Listeners::new`](../crates/web/src/browser/events.rs) registers the native
event set against the document/root and routes it into one dispatcher. That
dispatcher handles rich editing, selection/composition, drag/drop, menus,
grid/tree/choice navigation and other semantic interests. Runtime checks skip
irrelevant work, but do not make its machine code unreachable to the linker.

The renderer's common `apply`/`drain` paths also directly reference layers,
sheets, rich editors, files, clipboard, images and motion. Shared mapper and
semantic validation match all finite variants. Thus, not constructing a Sapid
widget can eliminate its builder/model code while its host-side capability
remains reachable through this common interpreter.

This explains why unused Sapid features can cost zero bytes while the empty
host still has a substantial floor. The named reactive-runtime functions alone
are not the dominant bucket, though their generic collection/glue costs are
also charged under the standard-library buckets.

## Collections, formatting and data

A second, disjoint classification of the same function bodies identifies:

| Structural implementation family | Bytes | Defined bodies |
|---|---:|---:|
| `alloc::collections::btree` implementations | 135,730 | 548 |
| Formatting / number-conversion symbols | 29,413 | 91 |
| Destruction glue | 24,960 | 246 |
| `hashbrown` implementations | 12,302 | 99 |
| `Clone` implementations | 9,193 | 33 |
| `dlmalloc` implementation | 7,275 | 6 |

The classification assigns destruction first, then BTree/hash implementations,
then formatting, allocator and clones. The rows above do not include all inlined
instances of these operations. Their bytes overlap the enclosing-symbol table,
so the tables must not be summed.

The many typed BTree maps/sets instantiate insert/remove/rebalancing code for
different payloads: nodes, event interests, subscriptions, property maps, query
tokens and runtime bookkeeping. For example, distinct retained insert bodies
exist for `BTreeMap<&str, Property>` and several maps keyed by `NodeId`. This
is a concrete target for shared internal representations, not evidence that
the whole collection family can be deleted or blindly replaced by HashMaps.
Ordering, operation complexity, Owner/query indices and cleanup bounds still
need to be preserved.

The data segments contain 78,173 B of actual initialized data in 32 segments.
51,223 B are printable ASCII runs of at least eight characters. Examples include
HTML/ARIA/property names, shared base CSS, detailed semantic validation errors
and paths. The remainder also contains binary data such as tables and vtables.
Those ASCII runs are not all error strings or dead debugging data. Even deleting
the entire data section would leave over 815 KB of code and other sections.

## Regular expressions: absent from this browser build

The resolved minimal target feature tree and the fixture lockfile contain no
`regex`, `regex-automata`, `regex-syntax`, `aho-corasick` or `fancy-regex`.
There are no regular-expression engine symbols in the named final module, and
the generated JS/snippets do not reference `RegExp`. This is absence from the
graph, not a claim that a Rust regex engine was included and then compressed
to a negligible size.

The repository does retain other regex references:

- The workspace dependency declaration `regex = ...` does not itself add a
  dependency to a package. The main workspace lockfile includes regex through
  the native server dependency `salvo_core`; that lockfile is not the minimal
  browser application's resolved graph.
- `crates/core/src/config.rs` is a legacy source using Rust regex to find config
  sections. The active `glory-core` module tree is arena/resource/runtime and
  its manifest has no regex dependency; it does not compile that file.
- The old `crates/routing` sources use Rust regex on native targets and a
  `js_sys::RegExp` wrapper on WASM. That package is not an active workspace
  member or facade dependency. It is an extraction reference, not current
  browser routing support or the mechanism used by this minimum app.
- Sapid's [`MaskPattern`](../../sapid/src/mask_model.rs) is a bounded ASCII
  parser (`#`, `A`, `*`, escaped literals), not general regex. Its
  [`code_model`](../../sapid/src/code_model.rs) performs bounded handwritten
  lexical scanning for Plain/Rust/JSON. Neither embeds a regex VM; the minimum
  case does not depend on Sapid at all.

## Next optimization targets

The attribution identifies possible size targets, not approved design changes.
The subsequent [code-quality review](wasm-size-quality-review.md) found no
independent quality justification for a new host-capability selection mechanism
or merging unrelated typed collections. Those changes are not being implemented.
The domains already have module boundaries, and generic collection instances
are not duplicate handwritten algorithms.

Keep the explicit renderer coordination and typed collections. Revisit them
for concrete ownership, correctness, maintenance or host requirements rather
than an initial-download target. Ordinary removal of demonstrated redundant
work or shared invariant logic can still be valuable, but the body-size tables
do not establish that redundancy. The allocator itself is only 7,275 B of named
function bodies; switching allocators or hunting for regex cannot address the
main floor here. Potential changes must preserve behavior and be measured with
the same matrix; these counts are not removable-size estimates.

## Reproduce without recompiling Rust

Use the pinned Binaryen and archived artifacts from the baseline:

```powershell
$env:TEMP = 'D:/BuildCache/sapid/tmp'
$env:TMP = $env:TEMP
$sizeRoot = 'D:/BuildCache/sapid/wasm-size'
New-Item -ItemType Directory -Force "$sizeRoot/analysis" | Out-Null
node "$sizeRoot/tools/node_modules/binaryen/bin/wasm-opt" "$sizeRoot/runs/optimized/glory-minimal/bindgen.wasm" -Oz --enable-bulk-memory --enable-reference-types -g -o "$sizeRoot/analysis/glory-minimal-named.wasm"
node benchmarks/wasm-size/attribute.cjs "$sizeRoot/analysis/glory-minimal-named.wasm" "$sizeRoot/runs/optimized/glory-minimal/optimized.wasm" > "$sizeRoot/analysis/attribution.json"
```

`attribute.cjs` rejects the comparison if code or other non-custom sections
differ. This analysis changed only tooling/documentation, not the production
renderer or its feature set.
