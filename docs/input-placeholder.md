# Bounded input placeholders

`Semantics::placeholder: Option<String>` is an opt-in presentation hint on an
Input, including multiline Input. It is independent from accessible name,
value/default value, revision, UTF-16 selection and composition. It never supplies
a semantic label or writes a business source. Hosts may expose their native
fallback hints; applications should still supply a real label. None omits/removes the attribute;
Some(empty) explicitly keeps an empty hint.

The shared `validate_input_placeholder` helper enforces 4096 UTF-8 bytes, no NUL
or CR, and no LF on a single-line Input. Multiline hints accept LF. Validation is
descriptor-local, before allocation or model/queue changes. Other node kinds,
including Select (which has its own selection placeholder), reject this metadata.
Prepared Semantics patches retain the normal rollback/sharing/prefix contracts.

HTML creation/snapshot/hydration and prepared-field patch lowering share the
same finite attribute mapping. Updating a hint emits only an attribute edit;
it does not lower an input value/property, revision, name, type or editor reset.
The test-only full-shape difference remains the differential oracle. Classic SSR
streaming carries this same finite attribute and uses the current HTML format;
there is no new node or arbitrary attribute escape hatch.

Native Blitz accepts the same finite metadata. The pinned organization fork
keeps one bounded current hint and lazily shapes an independent presentation
layout only for an empty, non-composing editor. Filled/composing editors release
that shaped layout; clearing the attribute releases the hint. Updating a hint
never replaces the editable value, selection, revision, password projection or
active IME. No all-input registry, business-source traversal or unsafe bridge is
introduced.

Hints inherit font, direction and alignment, preserve line breaks for multiline
Inputs, and paint neutral gray within the content box. Single-line hints never
wrap; multiline hints use the available content width. They do not inherit the
real editor's scrolling. Clearing a long value resets its old scroll offset.
Selection, pointer geometry, clipboard and IME always use the real editor (or
its existing password mask), never hint bytes. The native `:placeholder-shown`
state tracks current editor emptiness/composition and attribute presence, including
an explicitly empty hint. Attribute and state selectors retain normal Stylo
invalidation; a hint-only mutation does not force an editor rebuild.

AccessKit exports the hint as its separate placeholder property; it never
substitutes the accessible name or value. Password values remain absent. This
is tree/action coverage on desktop; mobile accessibility bridges and OS spoken
announcements remain separate gaps. Arbitrary `::placeholder` pseudo-element
styling is not included in this finite native hint policy.

Tests: `render/tests/placeholder.rs`, `html/src/placeholder_tests.rs`,
SSR `choice_stream_retains_focus_preserving_intrinsics_and_live_active_descendant`,
and `blitz/tests/placeholder.rs`. Sapid uses the shared validator and maps reactive
hints through this metadata in its basic text, choice, tag and JSON input APIs.
