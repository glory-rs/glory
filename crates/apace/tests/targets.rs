#![cfg(not(target_arch = "wasm32"))]

#[test]
fn native_target_contract_uses_real_boxes_and_rejects_boxless_geometry() {
    let runtime = glory_render::RuntimeId::new();
    glory_render::conformance::verify(glory_apace::Renderer::new(runtime), runtime, true);
}
