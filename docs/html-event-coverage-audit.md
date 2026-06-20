# HTML and Event Coverage Audit

Date: 2026-06-11

Reference: local Dioxus checkout at `E:\Repos\dioxus`, package
`packages/html`.

## Method

The audit compares public DOM-facing coverage, not internal implementation
names:

- Elements: extracted Dioxus builder tags from `packages/html/src/elements.rs`
  and compared them with Glory's generated HTML, SVG, and MathML builders by
  real DOM tag name.
- Events: extracted Dioxus `on* => event` entries plus concrete `#[raw = [...]]`
  entries from `packages/html/src/events/generated.rs`, then compared them with
  Glory's event descriptor names.

## Element Coverage

Result after this pass:

| Surface | Dioxus | Glory | Missing in Glory | Extra in Glory |
| --- | ---: | ---: | --- | --- |
| Real DOM tags | 202 | 204 | none | `html`, `portal` |

Changes made in Glory:

- `generate_tags!` now supports a snake-case Rust function name mapped to an
  explicit DOM tag name and namespace.
- CSR SVG and MathML builders now use `create_element_ns` instead of creating
  namespaced elements as plain HTML elements.
- SVG coverage now includes Dioxus' camel-case and edge tags such as
  `animateMotion`, `clipPath`, `linearGradient`, `foreignObject`, `textPath`,
  `feDropShadow`, `feComponentTransfer`, `switch`, `use`, and the filter
  primitive family.
- MathML coverage now includes Dioxus' remaining MathML tags such as
  `annotation-xml`, `merror`, `mmultiscripts`, `mpadded`, `mphantom`,
  `mprescripts`, `mroot`, `ms`, `mspace`, and `mstyle`.

Glory uses snake-case builder names for Rust ergonomics where the DOM tag is
not snake case or is a Rust keyword:

| Glory function | DOM tag |
| --- | --- |
| `svg::animate_motion()` | `animateMotion` |
| `svg::animate_transform()` | `animateTransform` |
| `svg::clip_path()` | `clipPath` |
| `svg::foreign_object()` | `foreignObject` |
| `svg::linear_gradient()` | `linearGradient` |
| `svg::radial_gradient()` | `radialGradient` |
| `svg::switch_()` | `switch` |
| `svg::text_path()` | `textPath` |
| `svg::use_()` | `use` |
| `math::annotation_xml()` | `annotation-xml` |

## Event Coverage

Result after this pass:

| Surface | Dioxus | Glory | Missing in Glory |
| --- | ---: | ---: | --- |
| Event names including Dioxus raw aliases | 99 | 140 | none |

Changes made in Glory:

- Added Dioxus DOM/raw gaps that fit Glory's current event descriptor model:
  `doubleclick`, `dragexit`, `encrypted`, `interruptbegin`, `interruptend`,
  `loadend`, and `timeout`.
- Added synthetic lifecycle descriptors: `mounted` and `visible`.
- `mounted` fires automatically on CSR after the DOM insertion tick and on the
  command backend before the initial or update command batch is drained.
- `visible` uses `IntersectionObserver` on CSR. Command/native/LiveView hosts
  expose the same event name and may dispatch it when their own visibility
  engine reports an intersecting/visible node.
- Clipboard events now use `web_sys::ClipboardEvent` for `copy`, `cut`, and
  `paste`.
- Corrected delegation metadata:
  `focusin` and `focusout` bubble; `mouseenter`, `mouseleave`, `pointerenter`,
  and `pointerleave` do not.

Remaining event deltas: none in the name/descriptor surface. Runtime payload
richness still depends on what each host can observe.

## Typed Payload Gap

Glory's browser CSR path intentionally gives handlers the native `web_sys`
event type. The command-stream backend gives handlers serializable
`renderer::EventData`, currently covering pointer, keyboard, target value,
target checked state, clipboard text, selection ranges/text, scroll offsets,
resize dimensions, and media state.

Dioxus' `packages/html` exposes platform-independent payload structs for more
event families. Glory now has the serializable payload shape for common browser
families plus lifecycle event names. `mounted` is automatic in CSR and
command-stream builds; `visible` is automatic in CSR and host-dispatched in
command/native/LiveView builds.

## Verification

Commands run:

- `cargo test -p glory-core --lib`
- `cargo test -p glory-core --lib --features web-ssr`
- `cargo test -p glory-core --features backend-command --test command_backend`
- `cargo check --manifest-path examples/forms-showcase/Cargo.toml --target wasm32-unknown-unknown`
- `cargo clippy -p glory-core --lib --features web-ssr -- -D warnings`
