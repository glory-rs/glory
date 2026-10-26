fn main() -> std::process::ExitCode {
    glory_cli::run_from_manifest(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"))
}
