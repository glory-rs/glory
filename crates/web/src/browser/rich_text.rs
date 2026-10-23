use super::backend;
use glory_render::{
    Error,
    rich_text::{Document, Selection},
};
use wasm_bindgen::prelude::*;
use web_sys::Element;

#[wasm_bindgen(module = "/src/browser/rich_text.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = setMarksRich)]
    fn marks_js(root: &Element, marks: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = beforeInputRich)]
    fn before_js(root: &Element, event: &web_sys::Event) -> Result<bool, JsValue>;
    #[wasm_bindgen(catch, js_name = compositionRich)]
    fn compose_js(root: &Element) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = finishCompositionRich)]
    fn finish_compose_js(root: &Element, event: &web_sys::Event) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = readRich)]
    fn read_js(root: &Element) -> Result<String, JsValue>;
    #[wasm_bindgen(catch, js_name = writeRich)]
    fn write_js(root: &Element, document: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = selectionRich)]
    fn selection_js(root: &Element) -> Result<String, JsValue>;
    #[wasm_bindgen(catch, js_name = setSelectionRich)]
    fn set_selection_js(root: &Element, selection: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = taskRich)]
    fn task_js(root: &Element, event: &web_sys::Event) -> Result<bool, JsValue>;
    #[wasm_bindgen(catch, js_name = pasteRich)]
    fn paste_js(root: &Element, event: &web_sys::Event) -> Result<bool, JsValue>;
}
fn parse(value: &str) -> Result<serde_json::Value, Error> {
    if value.len() > 8 * 1024 * 1024 {
        return Err(Error::InvalidEvent("rich DOM payload exceeds budget"));
    }
    serde_json::from_str(value).map_err(|error| Error::Backend(error.to_string()))
}
pub(super) fn read(root: &Element) -> Result<(Document, Selection), Error> {
    let value = parse(&read_js(root).map_err(backend)?)?;
    let document = glory_html::rich_text::wire::decode(&value["document"])?;
    let selection = glory_html::rich_text::wire::decode_selection(&value["selection"])?;
    document.validate_selection(selection)?;
    Ok((document, selection))
}
pub(super) fn write(root: &Element, document: &Document) -> Result<(), Error> {
    document.validate()?;
    write_js(
        root,
        &glory_html::rich_text::wire::encode(document).to_string(),
    )
    .map_err(backend)
}
pub(super) fn selection(root: &Element) -> Result<Selection, Error> {
    glory_html::rich_text::wire::decode_selection(&parse(&selection_js(root).map_err(backend)?)?)
}
pub(super) fn set_selection(root: &Element, selection: Selection) -> Result<(), Error> {
    set_selection_js(
        root,
        &glory_html::rich_text::wire::encode_selection(selection).to_string(),
    )
    .map_err(backend)
}
pub(super) fn paste(root: &Element, event: &web_sys::Event) -> Result<bool, Error> {
    paste_js(root, event).map_err(backend)
}

pub(super) fn task(root: &Element, event: &web_sys::Event) -> Result<bool, Error> {
    task_js(root, event).map_err(backend)
}

pub(super) fn set_marks(
    root: &Element,
    marks: &glory_render::rich_text::Marks,
) -> Result<(), Error> {
    marks.validate()?;
    let json = serde_json::json!({"bold":marks.bold,"italic":marks.italic,"strike":marks.strike,"code":marks.code,"link":marks.link});
    marks_js(root, &json.to_string()).map_err(backend)
}
pub(super) fn before(root: &Element, event: &web_sys::Event) -> Result<bool, Error> {
    before_js(root, event).map_err(backend)
}
pub(super) fn compose(root: &Element) -> Result<(), Error> {
    compose_js(root).map_err(backend)
}

pub(super) fn finish_compose(root: &Element, event: &web_sys::Event) -> Result<(), Error> {
    finish_compose_js(root, event).map_err(backend)
}
