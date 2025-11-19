use std::path::PathBuf;
use std::process::Command;

use crate::TappletConfig;
use anyhow::{Context, Result, bail};

pub struct LocalFolderTapplet {
    path: PathBuf,
    config: TappletConfig,
}

impl LocalFolderTapplet {
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
        println!("Installing tapplet: {}", self.config.name);

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

        // Compile the code from rust to wasm32-unknown-unknown
        println!("Compiling tapplet to WASM...");
        let output = Command::new("cargo")
            .current_dir(&self.path)
            .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
            .output()
            .context("Failed to execute cargo build. Is cargo installed?")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to compile tapplet:\n{}", stderr);
        }

        println!("Compilation successful!");

        // Find the compiled WASM file
        // The WASM file should be in target/wasm32-unknown-unknown/release/
        let wasm_target_dir = self
            .path
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("release");

        // Find .wasm files in the target directory
        let wasm_files: Vec<_> = std::fs::read_dir(&wasm_target_dir)
            .with_context(|| {
                format!(
                    "Failed to read WASM target directory: {}",
                    wasm_target_dir.display()
                )
            })?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "wasm")
                    .unwrap_or(false)
            })
            .collect();

        if wasm_files.is_empty() {
            bail!(
                "No WASM file found in target directory: {}",
                wasm_target_dir.display()
            );
        }

        // Use the first WASM file found (or we could use the package name to find the right one)
        let wasm_source = wasm_files[0].path();
        let wasm_target = target_path.join(format!("{}.wasm", self.config.name));

        println!(
            "Copying WASM file: {} -> {}",
            wasm_source.display(),
            wasm_target.display()
        );
        std::fs::copy(&wasm_source, &wasm_target).with_context(|| {
            format!(
                "Failed to copy WASM file from {} to {}",
                wasm_source.display(),
                wasm_target.display()
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
            "Successfully installed tapplet to: {}",
            target_path.display()
        );
        Ok(())
    }
}
