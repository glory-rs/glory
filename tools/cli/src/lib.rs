//! Project-owned Glory tooling. Link this library from a small private workspace
//! package; CLI dependencies never belong to the application's runtime graph.
mod args;
mod build;
mod process;
mod project;
mod scaffold;
mod serve;
#[cfg(test)]
mod test_support;

use std::path::Path;
use std::process::ExitCode;

type Result<T> = std::result::Result<T, String>;

/// Run the command line for an application's manifest. A private workspace
/// executable can pass `concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml")`.
/// Returns an exit code without terminating the process or hiding errors.
pub fn run_from_manifest(manifest: impl AsRef<Path>) -> ExitCode {
    let args = match args::Args::parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("glory: {error}");
            return ExitCode::FAILURE;
        }
    };
    match execute(manifest.as_ref(), args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("glory: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(manifest: &Path, args: args::Args) -> Result<()> {
    let manifest = args.manifest.as_deref().unwrap_or(manifest);
    if args.command == args::Command::Help {
        println!("{}", args::HELP);
        return Ok(());
    }
    if args.command == args::Command::Version {
        println!("glory-cli {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.command == args::Command::Init {
        return scaffold::init(manifest, args.force, args.cli_path.as_deref());
    }
    let project = project::Project::load(manifest, &args)?;
    match args.command {
        args::Command::Doctor => project.doctor(),
        args::Command::Print => project.print(&args),
        args::Command::Build => build::build(&project, &args).map(|_| ()),
        args::Command::Bundle => build::bundle(&project, &args),
        args::Command::Run => build::run(&project, &args),
        args::Command::Serve => serve::serve(project, args),
        args::Command::Check => project.check(&args),
        args::Command::Fmt => project.format(&args),
        _ => unreachable!(),
    }
}
