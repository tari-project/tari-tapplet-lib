use std::path::PathBuf;

use crate::TappletConfig;
use anyhow::{Context, Result, bail};

pub struct LocalFolderLuaTapplet {
    path: PathBuf,
    pub config: TappletConfig,
}

impl LocalFolderLuaTapplet {
    pub fn load(path: PathBuf) -> Result<Self> {
        let manifest_file = path.join("manifest.toml");
        if !manifest_file.exists() {
            bail!(
                "No manifest.toml found in the specified directory: {}",
                path.display()
            );
        }
        let config = TappletConfig::from_file(&manifest_file)?;

        Ok(Self { path, config })
    }

    pub fn install(&self, cache_directory: PathBuf) -> Result<()> {
        println!("Installing Lua tapplet: {}", self.config.name);

        // Create the target directory path: cache_directory/tapplet_name
        let target_path = cache_directory.join(&self.config.name);

        // Check if the directory already exists
        if target_path.exists() {
            println!("Tapplet already installed at: {}", target_path.display());
            return Ok(());
        }

        // Create the target directory
        std::fs::create_dir_all(&target_path).with_context(|| {
            format!(
                "Failed to create target directory: {}",
                target_path.display()
            )
        })?;

        // Find the Lua file in the source directory
        // Look for .lua files in the root of the tapplet directory
        let lua_files: Vec<_> = std::fs::read_dir(&self.path)
            .with_context(|| format!("Failed to read source directory: {}", self.path.display()))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "lua")
                    .unwrap_or(false)
            })
            .collect();

        if lua_files.is_empty() {
            bail!(
                "No Lua file found in source directory: {}",
                self.path.display()
            );
        }

        // Use the first Lua file found (or we could use the package name to find the right one)
        let lua_source = lua_files[0].path();
        let lua_target = target_path.join(format!("{}.lua", self.config.name));

        println!(
            "Copying Lua file: {} -> {}",
            lua_source.display(),
            lua_target.display()
        );
        std::fs::copy(&lua_source, &lua_target).with_context(|| {
            format!(
                "Failed to copy Lua file from {} to {}",
                lua_source.display(),
                lua_target.display()
            )
        })?;

        // Copy the manifest.toml
        let manifest_source = self.path.join("manifest.toml");
        let manifest_target = target_path.join("manifest.toml");

        println!(
            "Copying manifest: {} -> {}",
            manifest_source.display(),
            manifest_target.display()
        );
        std::fs::copy(&manifest_source, &manifest_target).with_context(|| {
            format!(
                "Failed to copy manifest from {} to {}",
                manifest_source.display(),
                manifest_target.display()
            )
        })?;

        println!(
            "Successfully installed Lua tapplet to: {}",
            target_path.display()
        );
        Ok(())
    }
}
