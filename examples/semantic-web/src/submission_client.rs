//! Real browser host-batch fault/growth/lifecycle probe using public Plan APIs.
use glory::prelude::*;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

struct Probe {
    web: glory::web::WebApp,
    owner: Scope,
    declaration: Plan,
    root: PlanPart,
    text: PlanPart,
    rows: Vec<PlanInstance>,
}
thread_local! { static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) }; }
fn js(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[wasm_bindgen]
pub fn submission_probe(stage: u8) -> std::result::Result<usize, JsValue> {
    if stage == 0 {
        PROBE.with(|probe| probe.borrow_mut().take());
        crate::browser::unmount();
        crate::plan_client::clear();
        let root = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("app")
            .unwrap();
        let web = glory::web::launch(
            root,
            input().class("submit-editor").value("a🙂b".to_owned()),
        )
        .map_err(js)?;
        let owner = web.app().scope().child().map_err(js)?;
        let declaration = plan(
            view()
                .class("submit-row")
                .mark("root")
                .fill((text().mark("text"), button().label("press"))),
        )
        .map_err(js)?;
        let root = declaration.part("root").map_err(js)?;
        let text = declaration.part("text").map_err(js)?;
        PROBE.with(|probe| {
            *probe.borrow_mut() = Some(Probe {
                web,
                owner,
                declaration,
                root,
                text,
                rows: Vec::new(),
            })
        });
        return Ok(0);
    }
    if stage == 9 {
        PROBE.with(|probe| probe.borrow_mut().take());
        return Ok(0);
    }
    PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let probe = probe.as_mut().ok_or_else(|| js("probe not mounted"))?;
        match stage {
            1 => {
                for index in 0..4 {
                    let row = probe.owner.instantiate(&probe.declaration).map_err(js)?;
                    row.text(&probe.text, format!("row {index}")).map_err(js)?;
                    probe.rows.push(row);
                }
            }
            2 => {
                for (index, row) in probe.rows.iter().enumerate() {
                    row.text(&probe.text, format!("changed🙂 {index}"))
                        .map_err(js)?;
                }
            }
            3 => probe.owner.dispose(),
            4 => {
                probe.rows[0]
                    .text(&probe.text, "accepted prefix🙂".to_owned())
                    .map_err(js)?;
                probe.rows[1]
                    .class(&probe.root, "submit-row throw-here".to_owned())
                    .map_err(js)?;
                let row = probe.owner.instantiate(&probe.declaration).map_err(js)?;
                row.text(&probe.text, "unexecuted new row".to_owned())
                    .map_err(js)?;
                probe.rows[2]
                    .text(&probe.text, "unexecuted suffix".to_owned())
                    .map_err(js)?;
                probe.rows.push(row);
                assert!(probe.web.app().flush().is_err());
                assert!(matches!(
                    probe.web.app().flush(),
                    Err(glory::ui::Error::Faulted)
                ));
                return Ok(1);
            }
            5 => {
                // Partial retirement must clear only this member of the shared
                // physical bank, retaining sibling instances and lazy targets.
                let node = probe.rows[0].node(&probe.text).map_err(js)?;
                let victim = probe.web.app().scope().child().map_err(js)?;
                let parent = victim
                    .create(glory::render::Descriptor::new(glory::render::Props::View))
                    .map_err(js)?;
                probe.owner.place(node, Some(parent), None).map_err(js)?;
                probe.web.app().flush().map_err(js)?;
                victim.dispose();
            }
            _ => return Err(js("unknown submission probe stage")),
        }
        probe.web.app().flush().map_err(js)?;
        Ok(probe.web.app().stats().nodes)
    })
}
