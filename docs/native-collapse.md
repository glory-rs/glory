# Native intrinsic-height collapse

Blitz accepts the existing finite `CollapseConfig` and advertises
`Capabilities::collapse_completion`. No shared wire format or widget controller
changes are needed. Sapid's `collapsible(...).height_transition(...)` consumes the
same declaration and revisioned completion event as Web.

Native motion is explicit. The default is Reduce; select NoPreference when the
application wants animated presentation:

```rust
let host = glory::blitz::shell::WindowApp::with_config(glory::blitz::Config {
    motion: glory::render::MotionPreference::NoPreference,
    ..Default::default()
});
```

Painting hosts can change this through `Renderer::set_motion_preference`.
The service does not infer an OS preference. Duration is bounded to 0..=500 ms;
revisions do not wrap, and equal revisions require equal configurations.

## Height and lifetime

The host owns the outer intrinsic block's clipping and temporary numeric
min/max-height constraints. Stylo interpolates those constraints on the native
document clock; Taffy lays out the real content, updates following siblings and
clips the painted body. There is no per-frame Cage or second component state
machine. Root sizing/padding/border/shadow restrictions remain the shared
`CollapseConfig::validate_style` contract; put these on the content instead.

Initial declarations settle without entrance animation. A live accepted change
samples the current extent, including an interrupted transition. Reopening never
publishes an older close's completion. Content or viewport changes while opening
remeasure current native intrinsic layout and retarget the remaining interval;
they do not extend the original deadline. Completion releases the temporary
extent so later text wrapping, content growth and ordinary sizing remain live.
Settled closed targets have zero extent even if an unavailable ancestor left a
stale physical box behind.

Closing immediately hides the logical subtree, makes it inert and withdraws its
accessibility content. Only the retained clipped pixels finish shrinking. The
current revision produces at most one completion, including zero duration,
Reduce, unavailable ancestors, CSS hiding, zero viewport and suspension. Resume
does not replay a completed revision. Disposal retires the phase and its native
transition without callbacks to a dead Owner. Sapid decides when a matching
terminal event may release Unmount content; KeepMounted retains it.

Only currently declared native collapse nodes and their ancestor paths enter the
presentation table. No business-source scan, completed-revision history, UI Owner
capture, timer or cross-document state is retained. Static presentation does not
invalidate on clock changes. `needs_observe` includes ready terminal events,
not animation work waiting for a future host frame. Headless tests may drive the
same monotonic `advance_time` clock; an attached shell owns its clock exclusively.

## Engine retirement

The genuine Blitz fork is pinned to
[`ee125981e0e5a217e758be5c470df23cdd9e2fc3`](https://github.com/glory-rs/blitz/commit/ee125981e0e5a217e758be5c470df23cdd9e2fc3).
After style traversal, it retires animations/transitions only for current
animation members under `display:none`, preserving unrelated live animations.
Showing the subtree restarts its CSS animation with a fresh start time.
`visibility:hidden` and `display:contents` do not cancel animation. This follows
the [CSS animation display lifecycle](https://www.w3.org/TR/css-animations-1/Overview.html).

A regression exposed a separate block-to-contents cache error: children acquired
their new layout parent while the old transparent container retained a competing
child roster. The fork clears that roster and its old anonymous boxes. Repeated
block/contents cycles preserve the editor and keep current layout/paint references
live and anonymous allocations bounded. No Cargo-managed checkout is edited;
all active pins/locks and the provenance record move together. No new unsafe.

## Evidence and boundaries

`crates/blitz/tests/collapse.rs` checks shared revision/prefix/query conformance,
Suspend/Resume/disposal, real intermediate geometry, following flow, actual CPU
pixels, reversal, nested collapses, content growth, text wrapping at 2x scale,
Reduce, zero viewport, author CSS hiding, completion deduplication and no reactive
frame writes. Existing translation regressions remain independent.

The actual Sapid native consumer runs all four themes and both content policies,
checking current editor identity, focus return, matching close completion,
stale-event refusal, zero-duration cleanup and whole-tree retirement. Detailed
logs live under `/Volumes/Data/Caches/glory/native-collapse/` and
`native-collapse-*.log`.

On the current Apple host, use the approved environment and Python 3.11+ for the
provenance script:

```sh
export GLORY_BUILD_ROOT=/Volumes/Data/Caches/glory/apple
. scripts/apple-env.sh
cargo test --locked -p glory-blitz --all-features --test collapse --test translation
cargo test --locked -p glory-blitz --no-default-features --test collapse
cargo test --locked --manifest-path ../sapid/examples/native/Cargo.toml --test collapse_motion
python3 scripts/check-engine-forks.py --test
```

These are native engine/CPU/component and target-compilation checks. OS input,
screen-reader announcements, mobile device presentation, Sheet gestures, rich
editing and the report's other platform/application gaps remain separate.
