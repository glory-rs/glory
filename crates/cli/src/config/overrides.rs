use std::net::{IpAddr, SocketAddr};

use camino::Utf8PathBuf;

use super::project::{ProjectConfig, ProjectDefinition};

/// Programmatic overrides supplied by the embeddable [`crate::Glory`] builder.
///
/// Every field is optional: a `None` defers to whatever the `Cargo.toml`
/// `[package.metadata.glory]` / `[[workspace.metadata.glory]]` section
/// provides (or to the serde default), while a `Some` wins over the metadata.
///
/// For the `Vec`/`bool` fields, `Some(vec![])` is an *explicit* empty override
/// and is distinct from `None` (defer to metadata).
#[derive(Debug, Default, Clone)]
pub struct Overrides {
    /// Select a single project by name in a multi-project workspace.
    pub project: Option<String>,

    // ProjectDefinition
    pub name: Option<String>,
    pub bin_package: Option<String>,
    pub lib_package: Option<String>,

    // ProjectConfig scalars
    pub output_name: Option<String>,
    pub site_addr: Option<SocketAddr>,
    pub site_address: Option<IpAddr>,
    pub site_port: Option<u16>,
    pub site_root: Option<Utf8PathBuf>,
    pub site_pkg_dir: Option<Utf8PathBuf>,
    pub reload_port: Option<u16>,
    pub style_file: Option<Utf8PathBuf>,
    pub tailwind_input_file: Option<Utf8PathBuf>,
    pub tailwind_config_file: Option<Utf8PathBuf>,
    pub assets_dir: Option<Utf8PathBuf>,
    pub js_dir: Option<Utf8PathBuf>,
    pub browser_query: Option<String>,
    pub bin_target: Option<String>,
    pub bin_target_triple: Option<String>,
    pub bin_target_dir: Option<String>,
    pub bin_cargo_command: Option<String>,
    pub end2end_cmd: Option<String>,
    pub end2end_dir: Option<Utf8PathBuf>,

    // ProjectConfig vecs / bools
    pub features: Option<Vec<String>>,
    pub lib_features: Option<Vec<String>>,
    pub bin_features: Option<Vec<String>>,
    pub lib_default_features: Option<bool>,
    pub bin_default_features: Option<bool>,

    // Profiles
    pub lib_profile_dev: Option<String>,
    pub lib_profile_release: Option<String>,
    pub bin_profile_dev: Option<String>,
    pub bin_profile_release: Option<String>,
}

impl Overrides {
    /// `true` when no override has been set — the legacy [`crate::run`] path
    /// passes a default `Overrides`, so this keeps behaviour identical.
    pub fn is_empty(&self) -> bool {
        // Cheap structural check: compare against the default.
        // Done field-by-field to avoid requiring `PartialEq` on `SocketAddr`
        // semantics differing from ours.
        self.project.is_none()
            && self.name.is_none()
            && self.bin_package.is_none()
            && self.lib_package.is_none()
            && self.output_name.is_none()
            && self.site_addr.is_none()
            && self.site_address.is_none()
            && self.site_port.is_none()
            && self.site_root.is_none()
            && self.site_pkg_dir.is_none()
            && self.reload_port.is_none()
            && self.style_file.is_none()
            && self.tailwind_input_file.is_none()
            && self.tailwind_config_file.is_none()
            && self.assets_dir.is_none()
            && self.js_dir.is_none()
            && self.browser_query.is_none()
            && self.bin_target.is_none()
            && self.bin_target_triple.is_none()
            && self.bin_target_dir.is_none()
            && self.bin_cargo_command.is_none()
            && self.end2end_cmd.is_none()
            && self.end2end_dir.is_none()
            && self.features.is_none()
            && self.lib_features.is_none()
            && self.bin_features.is_none()
            && self.lib_default_features.is_none()
            && self.bin_default_features.is_none()
            && self.lib_profile_dev.is_none()
            && self.lib_profile_release.is_none()
            && self.bin_profile_dev.is_none()
            && self.bin_profile_release.is_none()
    }

    /// Apply name / package overrides onto a (possibly metadata-derived)
    /// project definition.
    pub fn apply_definition(&self, def: &mut ProjectDefinition) {
        if let Some(v) = &self.name {
            def.name = v.clone();
        }
        if let Some(v) = &self.bin_package {
            def.bin_package = v.clone();
        }
        if let Some(v) = &self.lib_package {
            def.lib_package = v.clone();
        }
    }

    /// Overlay the scalar / vec / profile overrides onto a parsed
    /// [`ProjectConfig`]. Called after `ProjectConfig::parse` (so it also wins
    /// over `.env` overlays) and before the `Project` is assembled.
    pub fn apply_config(&self, config: &mut ProjectConfig) {
        if let Some(v) = &self.output_name {
            config.output_name = v.clone();
        }
        if let Some(v) = self.site_addr {
            config.site_addr = v;
        }
        if let Some(v) = self.site_address {
            config.site_addr.set_ip(v);
        }
        if let Some(v) = self.site_port {
            config.site_addr.set_port(v);
        }
        if let Some(v) = &self.site_root {
            config.site_root = v.clone();
        }
        if let Some(v) = &self.site_pkg_dir {
            config.site_pkg_dir = v.clone();
        }
        if let Some(v) = self.reload_port {
            config.reload_port = v;
        }
        if let Some(v) = &self.style_file {
            config.style_file = Some(v.clone());
        }
        if let Some(v) = &self.tailwind_input_file {
            config.tailwind_input_file = Some(v.clone());
        }
        if let Some(v) = &self.tailwind_config_file {
            config.tailwind_config_file = Some(v.clone());
        }
        if let Some(v) = &self.assets_dir {
            config.assets_dir = Some(v.clone());
        }
        if let Some(v) = &self.js_dir {
            config.js_dir = Some(v.clone());
        }
        if let Some(v) = &self.browser_query {
            config.browser_query = v.clone();
        }
        if let Some(v) = &self.bin_target {
            config.bin_target = v.clone();
        }
        if let Some(v) = &self.bin_target_triple {
            config.bin_target_triple = Some(v.clone());
        }
        if let Some(v) = &self.bin_target_dir {
            config.bin_target_dir = Some(v.clone());
        }
        if let Some(v) = &self.bin_cargo_command {
            config.bin_cargo_command = Some(v.clone());
        }
        if let Some(v) = &self.end2end_cmd {
            config.end2end_cmd = Some(v.clone());
        }
        if let Some(v) = &self.end2end_dir {
            config.end2end_dir = Some(v.clone());
        }
        if let Some(v) = &self.features {
            config.features = v.clone();
        }
        if let Some(v) = &self.lib_features {
            config.lib_features = v.clone();
        }
        if let Some(v) = &self.bin_features {
            config.bin_features = v.clone();
        }
        if let Some(v) = self.lib_default_features {
            config.lib_default_features = v;
        }
        if let Some(v) = self.bin_default_features {
            config.bin_default_features = v;
        }
        if let Some(v) = &self.lib_profile_dev {
            config.lib_profile_dev = Some(v.clone());
        }
        if let Some(v) = &self.lib_profile_release {
            config.lib_profile_release = Some(v.clone());
        }
        if let Some(v) = &self.bin_profile_dev {
            config.bin_profile_dev = Some(v.clone());
        }
        if let Some(v) = &self.bin_profile_release {
            config.bin_profile_release = Some(v.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(site_addr: &str) -> ProjectConfig {
        serde_json::from_value(serde_json::json!({
            "site_addr": site_addr,
            "reload_port": 3001
        }))
        .unwrap()
    }

    #[test]
    fn partial_site_override_preserves_unspecified_addr_parts() {
        let mut config = config("127.0.0.1:8000");
        let overrides = Overrides {
            site_address: Some([0, 0, 0, 0].into()),
            site_port: Some(9000),
            ..Default::default()
        };

        overrides.apply_config(&mut config);

        assert_eq!(config.site_addr.to_string(), "0.0.0.0:9000");
    }

    #[test]
    fn partial_site_override_wins_after_full_site_addr() {
        let mut config = config("127.0.0.1:8000");
        let overrides = Overrides {
            site_addr: Some(([192, 168, 1, 10], 8080).into()),
            site_port: Some(9000),
            ..Default::default()
        };

        overrides.apply_config(&mut config);

        assert_eq!(config.site_addr.to_string(), "192.168.1.10:9000");
    }
}
