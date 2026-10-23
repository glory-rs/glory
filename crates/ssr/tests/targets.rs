#[test]
fn ssr_records_valid_interests_but_never_invents_physical_results() {
    let runtime = glory_render::RuntimeId::new();
    glory_render::conformance::verify(glory_ssr::Renderer::new(runtime), runtime, false);
}
