extern crate proc_macro;

use anyhow::Result;
use camino::Utf8PathBuf;
use diff::Patches;
use node::LNode;
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

pub mod diff;
pub mod node;
pub mod parsing;
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

#[derive(Debug, Clone, Default)]
pub struct ViewMacros {
    // keyed by original location identifier
    views: Arc<RwLock<HashMap<Utf8PathBuf, Vec<MacroInvocation>>>>,
}

impl ViewMacros {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_paths<T: AsRef<Path>>(&self, paths: &[T]) -> Result<()> {
        let mut views = HashMap::new();

        for path in paths {
            for entry in WalkDir::new(path).into_iter().flatten() {
                if entry.file_type().is_file() {
                    let path: PathBuf = entry.path().into();
                    let path = Utf8PathBuf::try_from(path)?;
                    if path.extension() == Some("rs") || path.ends_with(".rs") {
                        let macros = Self::parse_file(&path)?;
                        let entry = views.entry(path.clone()).or_default();
                        *entry = macros;
                    }
                }
            }
        }

        *self.views.write() = views;

        Ok(())
    }

    pub fn parse_file(path: &Utf8PathBuf) -> Result<Vec<MacroInvocation>> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let ast = syn::parse_file(&content)?;

        let mut visitor = ViewMacroVisitor::default();
        visitor.visit_file(&ast);
        let mut views = Vec::new();
        for view in visitor.views {
            let span = view.span();
            let id = span_to_stable_id(path, span.start().line);
            let tokens = view.tokens.clone().into_iter();
            // TODO handle class = ...
            let rsx = rstml::parse2(tokens.collect::<proc_macro2::TokenStream>())?;
            let template = LNode::parse_view(rsx)?;
            views.push(MacroInvocation { id, template })
        }
        Ok(views)
    }

    pub fn patch(&self, path: &Utf8PathBuf) -> Result<Option<Patches>> {
        let new_views = Self::parse_file(path)?;
        let mut lock = self.views.write();
        let diffs = match lock.get(path) {
            None => return Ok(None),
            Some(current_views) => {
                if current_views.len() == new_views.len() {
                    let mut diffs = Vec::new();
                    for (current_view, new_view) in current_views.iter().zip(&new_views) {
                        if current_view.id == new_view.id && current_view.template != new_view.template {
                            diffs.push((current_view.id.clone(), current_view.template.diff(&new_view.template)));
                        }
                    }
                    diffs
                } else {
                    return Ok(None);
                }
            }
        };

        // update the status to the new views
        lock.insert(path.clone(), new_views);

        Ok(Some(Patches(diffs)))
    }
}

#[derive(Debug, Clone, Default)]
pub struct HotFunctions {
    functions: Arc<RwLock<HashMap<Utf8PathBuf, Vec<FunctionInvocation>>>>,
}

impl HotFunctions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_paths<T: AsRef<Path>>(&self, paths: &[T]) -> Result<()> {
        let mut functions = HashMap::new();

        for path in paths {
            for entry in WalkDir::new(path).into_iter().flatten() {
                if entry.file_type().is_file() {
                    let path: PathBuf = entry.path().into();
                    let path = Utf8PathBuf::try_from(path)?;
                    if path.extension() == Some("rs") || path.ends_with(".rs") {
                        let invocations = Self::parse_file(&path)?;
                        functions.insert(path, invocations);
                    }
                }
            }
        }

        *self.functions.write() = functions;
        Ok(())
    }

    pub fn parse_file(path: &Utf8PathBuf) -> Result<Vec<FunctionInvocation>> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let ast = syn::parse_file(&content)?;

        let mut visitor = ReloadableFunctionVisitor::default();
        visitor.visit_file(&ast);

        let mut functions = Vec::new();
        for function in visitor.functions {
            let span = function.span();
            if let Some(id) = first_literal_string(function) {
                functions.push(FunctionInvocation { id, line: span.start().line });
            }
        }
        Ok(functions)
    }

    pub fn patch(&self, path: &Utf8PathBuf) -> Result<Option<FunctionReplacementBatch>> {
        let new_functions = Self::parse_file(path)?;
        let mut lock = self.functions.write();
        let Some(current_functions) = lock.get(path) else {
            lock.insert(path.clone(), new_functions);
            return Ok(None);
        };

        let ids_stable = current_functions
            .iter()
            .map(|function| &function.id)
            .eq(new_functions.iter().map(|function| &function.id));
        if !ids_stable || new_functions.is_empty() {
            lock.insert(path.clone(), new_functions);
            return Ok(None);
        }

        let replacements = new_functions
            .iter()
            .map(|function| FunctionReplacement {
                id: function.id.clone(),
                path: path.to_string(),
                line: function.line,
            })
            .collect();
        lock.insert(path.clone(), new_functions);

        Ok(Some(FunctionReplacementBatch { replacements }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionInvocation {
    pub id: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionReplacement {
    pub id: String,
    pub path: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionReplacementBatch {
    pub replacements: Vec<FunctionReplacement>,
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

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MacroInvocation {
    id: String,
    template: LNode,
}

impl std::fmt::Debug for MacroInvocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacroInvocation").field("id", &self.id).finish()
    }
}

#[derive(Default, Debug)]
pub struct ViewMacroVisitor<'a> {
    views: Vec<&'a Macro>,
}

impl<'ast> Visit<'ast> for ViewMacroVisitor<'ast> {
    fn visit_macro(&mut self, node: &'ast Macro) {
        let ident = node.path.get_ident().map(|n| n.to_string());
        if ident == Some("view".to_string()) {
            self.views.push(node);
        }

        // Delegate to the default impl to visit any nested functions.
        visit::visit_macro(self, node);
    }
}

pub fn span_to_stable_id(path: impl AsRef<Path>, line: usize) -> String {
    let file = path.as_ref().to_str().unwrap_or_default().replace(['/', '\\'], "-");
    format!("{file}-{line}")
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

        let functions = HotFunctions::parse_file(&path).unwrap();
        let ids = functions.into_iter().map(|function| function.id).collect::<Vec<_>>();

        assert_eq!(ids, vec!["row", "card"]);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn emits_function_replacements_when_ids_are_stable() {
        let path = write_temp_rs(
            "glory-hot-functions-patch",
            r#"fn view(registry: &Registry) { let _ = reloadable_fn!("row", registry, |value: i32| value + 1); }"#,
        );
        let functions = HotFunctions::new();
        functions.update_from_paths(&[path.parent().unwrap()]).unwrap();

        std::fs::write(
            &path,
            r#"fn view(registry: &Registry) { let _ = reloadable_fn!("row", registry, |value: i32| value + 10); }"#,
        )
        .unwrap();
        let patch = functions.patch(&path).unwrap().unwrap();

        assert_eq!(patch.replacements.len(), 1);
        assert_eq!(patch.replacements[0].id, "row");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
