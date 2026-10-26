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

The pinned native engine has no accepted placeholder editor projection. Blitz
therefore rejects placeholder-bearing creates and Semantics updates in both
preflight and direct apply, before facets/model changes, retaining any accepted
prefix. Plain Inputs remain supported. This is an explicit optional capability
limit, not evidence of native hint painting or assistive-technology acceptance.

Tests: `render/tests/placeholder.rs`, `html/src/placeholder_tests.rs`,
SSR `choice_stream_retains_focus_preserving_intrinsics_and_live_active_descendant`,
and `blitz/tests/placeholder.rs`. Sapid uses the shared validator and maps reactive
hints through this metadata in its basic text, choice, tag and JSON input APIs.
