use std::{env, fs, path::PathBuf};
fn main() {
    let source = "src/browser/rich_text.js";
    println!("cargo:rerun-if-changed={source}");
    // Use exactly the same finite DOM codec in the classic stream bootstrap
    // and in the ES module consumed by wasm-bindgen. No runtime eval/import.
    let body = fs::read_to_string(source).expect("rich document host source");
    let body = body.replace("export function ", "function ");
    let classic = format!(
        "(()=>{{const key=Symbol.for('glory.richtext.stream');if(globalThis[key])return;\n{body}\nglobalThis[key]=Object.freeze({{validateRich,writeRich,readRich,selectionRich,setSelectionRich,pasteRich,taskRich}});}})();\n"
    );
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").expect("build output")).join("rich_text_stream.js"),
        classic,
    )
    .expect("rich document classic host");
}
