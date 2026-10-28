# Finite vector graphics

`Props::Graphic(GraphicProps)` is a resource-free semantic image leaf. It carries
a positive viewport, at most 256 path commands, optional uniform fill/stroke,
and an optional accessible alternative. It carries no HTML, SVG string, font,
GPU object or callback. Commands are Move, Line, Cubic and Close; every segment
requires an open subpath, including after Close. Every coordinate is finite and
within +/-1,000,000. Viewport dimensions are in 0.000001..=1,000,000; stroke width
is in 0.000001..=10,000. Stroke caps are Butt, Round or Square, joins are round,
and fills use nonzero winding. Multiple colors compose separate graphic nodes.

The viewport is centered with uniform meet scaling in the content box, which
clips the path. An absent alternative explicitly marks decoration; decorative
graphics reject names, naming relations and explicit focus. A present alternative
is nonempty and at most 1024 UTF-8 bytes. An explicit accessible name overrides
the alternative, and clearing the name restores it. Graphics accept no semantic
children and cannot impersonate structural roles. `ui::graphic().drawing(Vary)`
uses the normal staged builder and field binding preflight.

`Patch::Graphic(Change<Box<GraphicProps>>)` replaces only drawing payload. Keep is a
no-op and Clear restores the empty decorative default. Memory's prepared guard
swaps only this field, checks geometry and cross-field constraints, and restores
the old field on rejection or drop. Generic style/name patches do not scan path
commands. Direct and App adapter checks retain successful prefix watermarks.

HTML emits real SVG namespace svg/path elements with finite generated attributes.
CSR creation, plan cloning, SSR, hydration and the classic stream use the same
namespace selection. The shared full-shape oracle checks drawing replacement,
clear/reset, alternatives, naming, paths and attributes. No SVG strings are
parsed. Classic stream retains its finite tag vocabulary and versioned shape.

Native paint uses the pinned engine's existing CustomWidget recording path and
AnyRender Bezier primitives, so ancestor clipping, opacity and stacking remain
inside the engine's paint traversal. The paint feature alone enables these
native dependencies; no-paint native explicitly rejects Graphic. Native rejects
nonzero graphic padding and border widths and all graphic object-fit declarations:
the current engine callback reports border-box dimensions but places its scene
at the content origin. Zero edges are valid; use a separate wrapper for decorated
frames. Raw author CSS is outside this typed graphic box promise.

`GraphicProps::animation` optionally requests `GraphicAnimation::Rotate` with a
period in 16..=60000 milliseconds. This is a finite host command, with no
component timer, frame callback or reactive write. HTML rotates the content path
around the viewport center using shared CSS; a dynamic reduced-motion media
query disables that rotation. Removing animation clears both its marker and
duration, as checked by the full-shape oracle and classic stream patch test.

Native `Config::motion` defaults to `MotionPreference::Reduce`, which paints a
visible static shape. Hosts may explicitly select `NoPreference` at construction
or through `Renderer::set_motion_preference`; automatic OS preference discovery
is not implemented. `Renderer::advance_time` drives a finite monotonic clock,
invalidates the visual revision and coalesces a host wake only for active motion.
The current live animated-node table checks semantic/physical visibility;
hidden or suspended drawings stop requesting frames and disposal removes their
membership. Engine detachment unloads widgets, so Place/Resume restores missing
drawings in the moved subtree without replacing their engine identities. There
is no visited-node history. A quarter-turn raster test proves path rotation and
zero component source writes, including ancestor clip/opacity and resume.

Focused evidence includes render command/budget unit tests, raw/prepared rollback
tests, UI initial/update preflight tests, HTML's differential field oracle and
SSR stream Shell/Patch/End tests. Native raster checks actual fill and curve
pixels, all three stroke caps, ancestor clip/opacity, scaling, replacement identity
and disposal, plus each unsupported box property and accepted-prefix rejection.
These are rendered pixel/accessibility tree checks, not display completion or
screen-reader announcement evidence.

`GraphicProps::transition` requests `GraphicTransition { duration_millis }` in
0..=500, mutually exclusive with loop animation. A full initial declaration paints
its endpoint. Subsequent transitioned paths require equal command count and
matching Move/Line/Cubic/Close topology, enforced by raw and prepared-field
validation before adapter writes. Default reset removes the transition and path;
equal writes/Keep preserve a current transition. UI construction seeds the complete
initial GraphicProps before descriptor/profile validation and identity allocation,
so a creation is not misread as morphing from an empty live path.

HTML interpolates the finite d property. Reduced motion and absence of the typed
transition marker use transition:none, which cancels already-running transitions;
changing duration alone is insufficient. Native retains only a current bounded
source path while transitioning, samples it on interruption, and releases it at
completion/cancellation. Target geometry stays in the current widget props. It
uses the existing host clock, visibility membership, visual revision and wake path;
no application frame task or writes are introduced. Native rendering is allocation
free per command; one bounded Vec is sampled only for an interrupted replacement.
Hidden/suspended/zero viewport/reduced motion cancels the transient source; Owner
retirement removes the live entry. Reactivation does not replay old motion.

Shared render transition tests, the field oracle and classic-stream config tests
cover rejection/reset. Native `tests/graphic_transition.rs` proves actual raster
interpolation, continuous interruption, equal/Keep behavior, final frame/idle clock,
reduction, clip/opacity and hidden/Owner cancellation. Sapid Burger uses this path
for three bars, with component/browser evidence in its `docs/burger.md`.
