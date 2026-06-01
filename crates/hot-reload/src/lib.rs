use anyhow::Result;
use camino::Utf8PathBuf;
use parking_lot::RwLock;
use proc_macro2::TokenTree;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};
use syn::{
    Macro,
    spanned::Spanned,
    visit::{self, Visit},
};
use walkdir::WalkDir;

pub mod relink;

pub use relink::{FunctionRegistry, ReloadableFn};

pub const HOT_RELOAD_JS: &str = include_str!("patch.js");

#[macro_export]
macro_rules! reloadable_fn {
    ($id:literal, $registry:expr, $func:expr) => {
        $registry.register($id, $func)
    };
    ($id:literal, owner = $owner:expr, $registry:expr, $func:expr) => {
        $registry.register_with_owner($owner, $id, $func)
    };
}

#[macro_export]
macro_rules! reloadable_view {
    ($id:literal, $registry:expr, $func:expr) => {
        $crate::reloadable_fn!($id, $registry, $func)
    };
    ($id:literal, owner = $owner:expr, $registry:expr, $func:expr) => {
        $crate::reloadable_fn!($id, owner = $owner, $registry, $func)
    };
}

#[derive(Debug, Clone, Default)]
pub struct HotReloadFunctions {
    markers_by_file: Arc<RwLock<HashMap<Utf8PathBuf, Vec<ReloadableFunctionMarker>>>>,
}

impl HotReloadFunctions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_paths<T: AsRef<Path>>(&self, paths: &[T]) -> Result<()> {
        let mut markers_by_file = HashMap::new();

        for path in paths {
            for entry in WalkDir::new(path).into_iter().flatten() {
                if entry.file_type().is_file() {
                    let path: PathBuf = entry.path().into();
                    let path = Utf8PathBuf::try_from(path)?;
                    if path.extension() == Some("rs") || path.ends_with(".rs") {
                        let markers = Self::parse_file(&path)?;
                        markers_by_file.insert(path, markers);
                    }
                }
            }
        }

        *self.markers_by_file.write() = markers_by_file;
        Ok(())
    }

    pub fn parse_file(path: &Utf8PathBuf) -> Result<Vec<ReloadableFunctionMarker>> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let ast = syn::parse_file(&content)?;

        let mut visitor = ReloadableFunctionVisitor::default();
        visitor.visit_file(&ast);

        let mut markers = Vec::new();
        for function in visitor.functions {
            let span = function.span();
            if let Some(function_id) = first_literal_string(function) {
                markers.push(ReloadableFunctionMarker {
                    function_id,
                    line_number: span.start().line,
                });
            }
        }
        Ok(markers)
    }

    pub fn patch(&self, path: &Utf8PathBuf) -> Result<Option<FunctionReloadBatch>> {
        let new_markers = Self::parse_file(path)?;
        let mut lock = self.markers_by_file.write();
        let Some(current_markers) = lock.get(path) else {
            lock.insert(path.clone(), new_markers);
            return Ok(None);
        };

        let ids_stable = current_markers
            .iter()
            .map(|function| &function.function_id)
            .eq(new_markers.iter().map(|function| &function.function_id));
        if !ids_stable || new_markers.is_empty() {
            lock.insert(path.clone(), new_markers);
            return Ok(None);
        }

        let reloads = new_markers
            .iter()
            .map(|function| FunctionReload {
                function_id: function.function_id.clone(),
                source_path: path.to_string(),
                line_number: function.line_number,
            })
            .collect();
        lock.insert(path.clone(), new_markers);

        Ok(Some(FunctionReloadBatch { reloads }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReloadableFunctionMarker {
    pub function_id: String,
    pub line_number: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionReload {
    pub function_id: String,
    pub source_path: String,
    pub line_number: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionReloadBatch {
    pub reloads: Vec<FunctionReload>,
}

#[derive(Default, Debug)]
pub struct ReloadableFunctionVisitor<'a> {
    functions: Vec<&'a Macro>,
}

impl<'ast> Visit<'ast> for ReloadableFunctionVisitor<'ast> {
    fn visit_macro(&mut self, node: &'ast Macro) {
        let ident = node.path.segments.last().map(|segment| segment.ident.to_string());
        if matches!(ident.as_deref(), Some("reloadable_fn" | "reloadable_view")) {
            self.functions.push(node);
        }

        visit::visit_macro(self, node);
    }
}

fn first_literal_string(node: &Macro) -> Option<String> {
    let first = node.tokens.clone().into_iter().next()?;
    let TokenTree::Literal(literal) = first else {
        return None;
    };
    serde_json::from_str(&literal.to_string()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp_rs(name: &str, source: &str) -> Utf8PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lib.rs");
        std::fs::write(&path, source).unwrap();
        Utf8PathBuf::try_from(path).unwrap()
    }

    #[test]
    fn parses_reloadable_function_macro_ids() {
        let path = write_temp_rs(
            "glory-hot-functions",
            r#"
            fn view(registry: &glory_hot_reload::FunctionRegistry) {
                let _ = glory_hot_reload::reloadable_fn!("row", registry, |value: i32| value + 1);
                let _ = reloadable_view!("card", registry, |value: i32| value + 2);
            }
            "#,
        );

        let functions = HotReloadFunctions::parse_file(&path).unwrap();
        let ids = functions.into_iter().map(|function| function.function_id).collect::<Vec<_>>();

        assert_eq!(ids, vec!["row", "card"]);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn reloadable_view_registers_builder_function() {
        let registry = FunctionRegistry::new();
        let view = reloadable_view!("card", registry, |value: i32| value + 2).unwrap();

        assert_eq!(view.id(), "card");
        assert_eq!(view.call(1), 3);
        registry.replace("card", |value: i32| value + 10).unwrap();
        assert_eq!(view.call(1), 11);
    }

    #[test]
    fn emits_function_replacements_when_ids_are_stable() {
        let path = write_temp_rs(
            "glory-hot-functions-patch",
            r#"fn view(registry: &Registry) { let _ = reloadable_fn!("row", registry, |value: i32| value + 1); }"#,
        );
        let functions = HotReloadFunctions::new();
        functions.update_from_paths(&[path.parent().unwrap()]).unwrap();

        std::fs::write(
            &path,
            r#"fn view(registry: &Registry) { let _ = reloadable_fn!("row", registry, |value: i32| value + 10); }"#,
        )
        .unwrap();
        let patch = functions.patch(&path).unwrap().unwrap();

        assert_eq!(patch.reloads.len(), 1);
        assert_eq!(patch.reloads[0].function_id, "row");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
