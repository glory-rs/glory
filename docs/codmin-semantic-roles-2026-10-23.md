# Codmin migration: structural table and dialog roles

Codmin's management tables and confirmations require semantic roles in addition to their CSS layout. The finite View `Role` set now includes `Table`, `Row`, `ColumnHeader`, `Cell`, and `Dialog`. These are structural View roles; intrinsic controls keep their intrinsic roles.

The change updates `crates/render/src/props.rs`, `crates/html/src/mapping.rs`, `crates/blitz/src/accessibility.rs`, `crates/html/src/patch_tests.rs`, and `crates/render/tests/semantics.rs`. HTML maps the roles to standard ARIA role names; the optional native AccessKit bridge maps the corresponding semantic roles. The existing HTML shape/patch differential oracle includes each new role, and render target validation tests cover their View-only restriction. No new wire format, HTML compatibility API, or runtime dependency was added.

Validation on 2026-10-23: `cargo test --locked -p glory-render -p glory-html` passed with `D:/BuildCache/codmin-glory/glory-target`; `cargo check --locked -p glory-blitz --features accesskit` passed with the existing `D:/BuildCache/glory-semantic-ui/target`. Both target directories were checked using Cargo metadata. Logs are in `D:/BuildCache/codmin-glory/logs/glory-roles.log` and `blitz-roles-check.log`.

Codmin's browser test inspects actual AX table/row/header/cell roles. Its external browser host implements modal focus, Escape, Tab cycling and inert siblings. `Role::Dialog` alone does not create those host policies, and native compilation does not establish OS assistive-technology acceptance. See `D:/Works/glory-rs/codmin-glory/docs/glory-migration.md` for application coverage and limitations.
