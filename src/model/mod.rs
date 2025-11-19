use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TappletConfig {
    pub name: String,
    pub version: String,
    pub friendly_name: String,
    pub description: String,
    pub publisher: String,
    pub git: GitConfig,
    pub api: ApiConfig,
    pub sigs: SigsConfig,
    pub public_key: String,
}

impl TappletConfig {
    pub fn canonical_name(&self) -> String {
        format!("{}@{}", self.name.replace("-", "_"), self.version)
    }

    pub fn name_matches(&self, other_name: &str) -> bool {
        self.name == other_name
            || self.name.replace("-", "_") == other_name
            || self.name.replace("_", "-") == other_name
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitConfig {
    pub url: String,
    pub rev: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiConfig {
    pub methods: Vec<String>,
    #[serde(flatten)]
    pub method_definitions: HashMap<String, MethodDefinition>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MethodDefinition {
    pub description: String,
    #[serde(default)]
    pub params: HashMap<String, ParamDefinition>,
    pub returns: ReturnDefinition,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParamDefinition {
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReturnDefinition {
    #[serde(rename = "type")]
    pub return_type: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SigsConfig {
    pub todo: String,
}

impl TappletConfig {
    /// Parse a tapplet configuration from a TOML string
    pub fn from_toml_str(toml_str: &str) -> Result<Self> {
        Ok(toml::from_str(toml_str)?)
    }

    /// Load a tapplet configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml_str(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_example_tapplet() {
        let toml_content = r#"
name = "password_manager"
version = "0.1.0"
friendly_name = "Password Manager"
description = "A simple password manager tapplet."
publisher = "a86b454a33b98f7f4f296a86dcbf08eaa816de5347d5c932b5fed8a95c52d04a"
public_key = "a86b454a33b98f7f4f296a86dcbf08eaa816de5347d5c932b5fed8a95c52d04a"
git = { url = "https://github.com/stringhandler/password_manager_tapplet", rev = "main" }

[api]
methods = ["greet"]

[api.greet]
description = "Returns a greeting message."

[api.greet.params]
name = { type = "string", description = "The name to greet." }

[api.greet.returns]
type = "string"
description = "A greeting message."

[sigs]
todo = "add sigs here"
"#;

        let config = TappletConfig::from_toml_str(toml_content).unwrap();

        assert_eq!(config.name, "password_manager");
        assert_eq!(config.version, "0.1.0");
        assert_eq!(config.friendly_name, "Password Manager");
        assert_eq!(
            config.git.url,
            "https://github.com/stringhandler/password_manager_tapplet"
        );
        assert_eq!(config.api.methods, vec!["greet"]);
        assert!(config.api.method_definitions.contains_key("greet"));
    }
}
