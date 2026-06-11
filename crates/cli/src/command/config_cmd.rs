use serde::Serialize;

use crate::config::{Config, Project};
use crate::ext::anyhow::Result;

pub async fn config_validate(conf: &Config, json: bool) -> Result<()> {
    let summary = ConfigSummary::from_config(conf);
    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("Glory config OK: {} project(s)", summary.projects.len());
        for project in &summary.projects {
            println!(
                "- {} target={} lib={} bin={} site_root={} site_pkg_dir={}",
                project.name, project.target, project.lib_package, project.bin_package, project.site_root, project.site_pkg_dir
            );
        }
    }
    Ok(())
}

pub async fn config_schema() -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&schema())?);
    Ok(())
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://glory.rs/schemas/glory-cli-config.schema.json",
        "title": "Glory Cargo metadata",
        "description": "Configuration accepted under [package.metadata.glory] or [[workspace.metadata.glory]]. Workspace entries additionally require name/bin-package/lib-package.",
        "sections": {
            "package.metadata.glory": {
                "type": "object",
                "required": [],
                "fields": project_config_fields(),
            },
            "workspace.metadata.glory": {
                "type": "array<object>",
                "required": ["name", "bin-package", "lib-package"],
                "fields": {
                    "name": {"type": "string", "required": true, "description": "Logical Glory project name."},
                    "bin-package": {"type": "string", "required": true, "description": "Workspace package that owns the server/host binary."},
                    "lib-package": {"type": "string", "required": true, "description": "Workspace package compiled for the client/mobile library."},
                    "config": project_config_fields()
                }
            }
        }
    })
}

fn project_config_fields() -> serde_json::Value {
    serde_json::json!({
        "output_name": {"type": "string", "default": "package/project name"},
        "site_addr": {"type": "socket address", "default": "127.0.0.1:8000"},
        "site_root": {"type": "path", "default": "target/site"},
        "site_pkg_dir": {"type": "path", "default": "pkg"},
        "style_file": {"type": "path", "optional": true},
        "tailwind_input_file": {"type": "path", "optional": true},
        "tailwind_config_file": {"type": "path", "optional": true},
        "assets_dir": {"type": "path", "optional": true},
        "js_dir": {"type": "path", "optional": true, "description": "Directory watched for JS-side changes."},
        "reload_port": {"type": "integer", "default": 3001},
        "end2end_cmd": {"type": "string", "optional": true},
        "end2end_dir": {"type": "path", "optional": true},
        "browser_query": {"type": "string", "default": "defaults"},
        "bin_target": {"type": "string", "optional": true},
        "bin_target_triple": {"type": "string", "optional": true},
        "bin_target_dir": {"type": "path", "optional": true},
        "bin_cargo_command": {"type": "string", "optional": true},
        "features": {"type": "array<string>", "default": []},
        "lib_features": {"type": "array<string>", "default": []},
        "lib_default_features": {"type": "boolean", "default": false},
        "bin_features": {"type": "array<string>", "default": []},
        "bin_default_features": {"type": "boolean", "default": false},
        "lib_profile_dev": {"type": "string", "optional": true},
        "lib_profile_release": {"type": "string", "optional": true},
        "bin_profile_dev": {"type": "string", "optional": true},
        "bin_profile_release": {"type": "string", "optional": true}
    })
}

#[derive(Serialize)]
struct ConfigSummary {
    working_dir: String,
    target: String,
    release: bool,
    hot_reload: bool,
    watch: bool,
    projects: Vec<ProjectSummary>,
}

impl ConfigSummary {
    fn from_config(conf: &Config) -> Self {
        Self {
            working_dir: conf.working_dir.to_string(),
            target: format!("{:?}", conf.cli.target).to_lowercase(),
            release: conf.cli.release,
            hot_reload: conf.cli.hot_reload,
            watch: conf.watch,
            projects: conf.projects.iter().map(|project| ProjectSummary::from_project(project)).collect(),
        }
    }
}

#[derive(Serialize)]
struct ProjectSummary {
    name: String,
    target: String,
    lib_package: String,
    bin_package: String,
    lib_features: Vec<String>,
    bin_features: Vec<String>,
    lib_default_features: bool,
    bin_default_features: bool,
    site_root: String,
    site_pkg_dir: String,
    site_addr: String,
    assets_dir: Option<String>,
    style_file: Option<String>,
    end2end_cmd: Option<String>,
}

impl ProjectSummary {
    fn from_project(project: &Project) -> Self {
        Self {
            name: project.name.clone(),
            target: format!("{:?}", project.target).to_lowercase(),
            lib_package: project.lib.name.clone(),
            bin_package: project.bin.name.clone(),
            lib_features: project.lib.features.clone(),
            bin_features: project.bin.features.clone(),
            lib_default_features: project.lib.default_features,
            bin_default_features: project.bin.default_features,
            site_root: project.site.root_dir.to_string(),
            site_pkg_dir: project.site.pkg_dir.to_string(),
            site_addr: project.site.addr.to_string(),
            assets_dir: project.assets.as_ref().map(|assets| assets.dir.to_string()),
            style_file: project.style.file.as_ref().map(|file| file.source.to_string()),
            end2end_cmd: project.end2end.as_ref().map(|end2end| end2end.cmd.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_includes_required_workspace_project_fields() {
        let schema = schema();
        let required = schema["sections"]["workspace.metadata.glory"]["required"].as_array().unwrap();
        assert!(required.iter().any(|value| value == "name"));
        assert!(required.iter().any(|value| value == "bin-package"));
        assert!(required.iter().any(|value| value == "lib-package"));
    }
}
