# Finite caption placement and border patterns

`Style::caption_side(CaptionSide::{Top,Bottom})` declares actual table-caption
formatting. Merely appending a caption after table sections does not make a
browser place it below the table. The HTML-family serializer emits the standard
`caption-side` declaration independently of all other style fields. Removing it
restores stylesheet/inherited behavior without replacing the table or caption.

`BorderStyle` additionally supports Dashed and Dotted. Border width, color and
style remain independent declarations; no shorthand resets absent fields.
The optional CSS parser accepts these finite keywords, preserves each original
declaration and rejects unimplemented caption/border keywords before installation.
No CSS parser enters default or reactive-patch graphs.

HTML and Record style profiles are version 14, and the native Apace profile is
version 15. Both support masks remain single `u64` values. Equivalent CSS
keywords use the canonical `Value::{Start,End,Center,NoWrap}` flags across their
align/justify/wrapping properties; retired duplicate flags have no aliases.
The finite value set remains 64 entries, with Ch at bit 63. Each using property
and value is independently checked. CaptionTop/CaptionBottom and dashed/dotted
borders have independent acceptance flags.

`crates/style` tests cover every property/value, original stylesheet declaration
rejection and the unchanged mask bounds. The HTML prepared/full-shape
differential patch oracle includes both caption sides, both border patterns and
their subsequent resets. These are CSS declarations inside the existing typed
style attribute protocol; the HTML shape/wire format remains version 19.

`crates/apace/tests/caption_border.rs` verifies actual pinned-engine caption
geometry for Bottom/Top/reset while retaining the physical caption. Its raster
test checks real dashed/dotted gaps and the same retained node through live
border changes. These are CPU/layout checks, not OS input or reader acceptance.
No new engine fork, unsafe code, dependency or implicit transport was introduced.
