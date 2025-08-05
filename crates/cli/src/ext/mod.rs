#[cfg(all(test, feature = "full_tests"))]
mod tests;

pub mod anyhow;
mod cargo;
pub mod exe;
pub mod fs;
mod path;
pub mod sync;
mod util;

pub use cargo::{MetadataExt, PackageExt};
pub use exe::{Exe, ExeMeta};
pub use path::{PathBufExt, PathExt, append_str_to_filename, determine_pdb_filename, remove_nested};
pub use util::{StrAdditions, os_arch};
