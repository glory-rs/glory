#![cfg(feature = "conformance")]

#[test]
fn reference_target_contract_has_no_physical_capabilities() {
    let runtime = glory_render::RuntimeId::new();
    glory_render::conformance::verify(glory_render::memory::Memory::new(runtime), runtime, false);
}
