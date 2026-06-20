use camino::Utf8PathBuf;

use crate::{
    config::{Cli, Commands, Opts},
    ext::PathBufExt,
    run,
};

#[tokio::test]
async fn workspace_build() {
    let command = Commands::Build(Opts::default());

    let cli = Cli {
        manifest_path: Some(workspace_root().join("examples/workspace/Cargo.toml")),
        log: Vec::new(),
        command,
    };

    run(cli).await.unwrap();

    let site_dir = workspace_root().join("examples/workspace/target/site");

    insta::assert_display_snapshot!(site_dir.ls_ascii(0).unwrap_or_default());
}

#[tokio::test]
async fn project_build() {
    let command = Commands::Build(Opts::default());

    let cli = Cli {
        manifest_path: Some(workspace_root().join("examples/project/Cargo.toml")),
        log: Vec::new(),
        command,
    };

    run(cli).await.unwrap();

    let site_dir = workspace_root().join("examples/project/target/site");

    insta::assert_display_snapshot!(site_dir.ls_ascii(0).unwrap_or_default());
}

fn workspace_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("glory-cli crate should live under crates/cli")
        .to_path_buf()
}
