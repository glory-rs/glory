use glory_render::*;
#[test]
fn ssr_stores_finite_motion_without_advertising_physical_completion() {
    let runtime = RuntimeId::new();
    let renderer = glory_ssr::Renderer::new(runtime);
    assert!(!renderer.capabilities().collapse_completion);
    conformance::verify_collapse(renderer, runtime, true);
}
