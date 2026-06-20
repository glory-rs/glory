use std::path::PathBuf;

use cargo_metadata::MetadataCommand;

fn fixture_manifest(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name).join("Cargo.toml")
}

#[test]
fn fixture_manifests_parse_as_cargo_packages() {
    for (fixture, expected_package) in [
        ("web", "glory-harness-web"),
        ("ssr", "glory-harness-ssr"),
        ("desktop", "glory-harness-desktop"),
        ("mobile", "glory-harness-mobile"),
    ] {
        let metadata = MetadataCommand::new()
            .manifest_path(fixture_manifest(fixture))
            .no_deps()
            .exec()
            .unwrap_or_else(|err| panic!("fixture `{fixture}` metadata failed: {err}"));
        assert_eq!(metadata.root_package().map(|package| package.name.as_str()), Some(expected_package));
    }
}
