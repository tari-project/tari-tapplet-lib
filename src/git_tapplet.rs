use std::path::PathBuf;

use anyhow::Result;

use crate::TappletManifest;

pub struct GitTapplet {
    config: TappletManifest,
}

impl GitTapplet {
    pub fn new(_config: TappletManifest) -> Self {
        todo!("Need to find a way to validate safely that this repo can be used");
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

        todo!("Implement git clone functionality here");
        // Clone the repository
        // println!("Cloning from: {}", self.config.git.url);
        // let repo = Repository::clone(&self.config.git.url, &target_path)
        //     .with_context(|| format!("Failed to clone repository from {}", self.config.git.url))?;

        // // Checkout the specific revision if specified
        // if !self.config.git.rev.is_empty() {
        //     println!("Checking out revision: {}", self.config.git.rev);

        //     // Find the object for the revision
        //     let oid = repo
        //         .revparse_single(&self.config.git.rev)
        //         .with_context(|| format!("Failed to find revision: {}", self.config.git.rev))?
        //         .id();

        //     // Get the object and peel it to a commit
        //     let object = repo.find_object(oid, None).with_context(|| {
        //         format!(
        //             "Failed to find object for revision: {}",
        //             self.config.git.rev
        //         )
        //     })?;

        //     // Checkout the specific revision
        //     repo.checkout_tree(&object, None)
        //         .with_context(|| format!("Failed to checkout revision: {}", self.config.git.rev))?;

        //     // Set HEAD to the detached state at this revision
        //     repo.set_head_detached(oid).with_context(|| {
        //         format!("Failed to set HEAD to revision: {}", self.config.git.rev)
        //     })?;
        // }

        // println!(
        // "Successfully installed tapplet to: {}",
        // target_path.display()
        // );
        // Ok(())
    }
}
