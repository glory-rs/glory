//! Development/bootstrap driver for the library-only package. This is not an
//! installed global CLI; applications own their own small executable.
fn main() -> std::process::ExitCode {
    glory_cli::run_from_manifest("Cargo.toml")
}
