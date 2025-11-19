use std::path::{Path, PathBuf};

use crate::TappletConfig;
use anyhow::{Context, Result};
use git2::{
    AutotagOption, FetchOptions as Git2FetchOptions, RemoteCallbacks, Repository,
    build::RepoBuilder,
};

pub struct TappletRegistry {
    pub name: String,
    pub git_url: String,
    pub cache_directory: PathBuf,
    pub current_revision: Option<String>,
    pub tapplets: Vec<TappletConfig>,
    is_loaded: bool,
}

impl TappletRegistry {
    pub fn new<S: AsRef<str>>(name: S, git_url: S, cache_directory: PathBuf) -> Self {
        Self {
            name: name.as_ref().to_string(),
            git_url: git_url.as_ref().to_string(),
            cache_directory,
            current_revision: None,
            tapplets: Vec::new(),
            is_loaded: false,
        }
    }

    pub fn revision(&self) -> Option<&String> {
        self.current_revision.as_ref()
    }

    /// Load tapplets from an already-fetched repository in the cache directory
    /// without performing a fetch operation.
    ///
    /// This is useful when you want to read the cached data without updating it.
    /// Returns an error if the repository hasn't been fetched yet.
    pub async fn load(&mut self) -> Result<()> {
        let git_url = self.git_url.clone();
        let cache_directory = self.cache_directory.clone();

        let result =
            tokio::task::spawn_blocking(move || Self::load_blocking(&git_url, &cache_directory))
                .await
                .context("Failed to spawn blocking task")??;

        // Update the registry with the loaded data
        self.current_revision = Some(result.commit_hash);
        self.tapplets = result.tapplets;
        self.is_loaded = true;

        Ok(())
    }

    /// Fetch or update the repository from the remote and load tapplets.
    ///
    /// This will clone the repository if it doesn't exist, or pull updates if it does.
    pub async fn fetch(&mut self) -> Result<()> {
        // Use tokio to run the blocking git operations in a separate thread
        let git_url = self.git_url.clone();
        let cache_directory = self.cache_directory.clone();

        let result =
            tokio::task::spawn_blocking(move || Self::fetch_blocking(&git_url, &cache_directory))
                .await
                .context("Failed to spawn blocking task")??;

        // Update the registry with the fetched data
        self.current_revision = Some(result.commit_hash);
        self.tapplets = result.tapplets;
        self.is_loaded = true;

        Ok(())
    }

    /// Blocking implementation of load for use with tokio::spawn_blocking
    fn load_blocking(git_url: &str, cache_directory: &Path) -> Result<FetchResult> {
        let repo_path = cache_directory.join(sanitize_repo_name(git_url));

        // Check if the repository exists
        if !repo_path.exists() {
            anyhow::bail!(
                "Repository not found at {}. Please fetch it first using fetch().",
                repo_path.display()
            );
        }

        // Open the repository
        let repository =
            Repository::open(&repo_path).context("Failed to open cached repository")?;

        // Get the current commit hash
        let head = repository.head().context("Failed to get HEAD reference")?;
        let commit = head
            .peel_to_commit()
            .context("Failed to peel HEAD to commit")?;
        let commit_hash = commit.id().to_string();

        // Parse all tapplet configurations from the repository
        let tapplets = parse_tapplets_from_repo(&repo_path)
            .context("Failed to parse tapplet configurations")?;

        Ok(FetchResult {
            repository_path: repo_path,
            was_cloned: false,
            commit_hash,
            tapplets,
        })
    }

    /// Blocking implementation of fetch for use with tokio::spawn_blocking
    fn fetch_blocking(git_url: &str, cache_directory: &Path) -> Result<FetchResult> {
        let repo_path = cache_directory.join(sanitize_repo_name(git_url));

        // Ensure cache directory exists
        if !cache_directory.exists() {
            std::fs::create_dir_all(cache_directory).context("Failed to create cache directory")?;
        }

        let repository;
        let was_cloned;

        // Check if the repository already exists
        if repo_path.exists() {
            // Repository exists, try to open and pull
            repository =
                Repository::open(&repo_path).context("Failed to open existing repository")?;
            fetch_updates(&repository).context("Failed to fetch updates")?;
            was_cloned = false;
        } else {
            // Clone the repository
            repository = clone_repository(git_url, &repo_path)
                .with_context(|| format!("Failed to clone repository from {}", git_url))?;
            was_cloned = true;
        }

        // Checkout main/master branch
        checkout_default_branch(&repository).context("Failed to checkout default branch")?;

        // Get the current commit hash
        let head = repository.head().context("Failed to get HEAD reference")?;
        let commit = head
            .peel_to_commit()
            .context("Failed to peel HEAD to commit")?;
        let commit_hash = commit.id().to_string();

        // Parse all tapplet configurations from the repository
        let tapplets = parse_tapplets_from_repo(&repo_path)
            .context("Failed to parse tapplet configurations")?;

        Ok(FetchResult {
            repository_path: repo_path,
            was_cloned,
            commit_hash,
            tapplets,
        })
    }

    pub fn search(&self, query: &str) -> Result<Vec<&TappletConfig>> {
        if !self.is_loaded {
            anyhow::bail!("Registry not loaded. Please call fetch() or load() first.");
        }
        let query_lower = query.to_lowercase();
        Ok(self
            .tapplets
            .iter()
            .filter(|tapplet| {
                tapplet.name.to_lowercase().contains(&query_lower)
                    || tapplet.friendly_name.to_lowercase().contains(&query_lower)
                    || tapplet.description.to_lowercase().contains(&query_lower)
                    || tapplet.publisher.to_lowercase().contains(&query_lower)
            })
            .collect())
    }

    pub fn tapplets_and_dirs(&self) -> Result<Vec<(&TappletConfig, PathBuf)>> {
        if !self.is_loaded {
            anyhow::bail!("Registry not loaded. Please call fetch() or load() first.");
        }
        let mut results = Vec::new();
        for tapplet in &self.tapplets {
            let dir = self
                .cache_directory
                .join(sanitize_repo_name(&self.git_url))
                .join("tapplets")
                .join(&tapplet.name);
            results.push((tapplet, dir));
        }
        Ok(results)
    }
}

struct FetchResult {
    #[allow(dead_code)]
    repository_path: PathBuf,
    #[allow(dead_code)]
    was_cloned: bool,
    commit_hash: String,
    tapplets: Vec<TappletConfig>,
}

/// Clone a repository from a URL to a local path
fn clone_repository(url: &str, path: &Path) -> Result<Repository> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            print!(
                "Resolving deltas {}/{}\r",
                stats.indexed_deltas(),
                stats.total_deltas()
            );
        } else if stats.total_objects() > 0 {
            print!(
                "Received {}/{} objects ({}) in {} bytes\r",
                stats.received_objects(),
                stats.total_objects(),
                stats.indexed_objects(),
                stats.received_bytes()
            );
        }
        std::io::Write::flush(&mut std::io::stdout()).ok();
        true
    });

    let mut fetch_options = Git2FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_options);

    let repo = builder.clone(url, path)?;
    println!(); // New line after progress
    Ok(repo)
}

/// Fetch updates from the remote repository
fn fetch_updates(repo: &Repository) -> Result<()> {
    let mut remote = repo
        .find_remote("origin")
        .or_else(|_| repo.remote_anonymous("origin"))?;

    let mut callbacks = RemoteCallbacks::new();
    callbacks.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            print!(
                "Resolving deltas {}/{}\r",
                stats.indexed_deltas(),
                stats.total_deltas()
            );
        } else if stats.total_objects() > 0 {
            print!(
                "Received {}/{} objects\r",
                stats.received_objects(),
                stats.total_objects()
            );
        }
        std::io::Write::flush(&mut std::io::stdout()).ok();
        true
    });

    let mut fetch_options = Git2FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    fetch_options.download_tags(AutotagOption::All);

    remote.fetch(
        &["refs/heads/*:refs/remotes/origin/*"],
        Some(&mut fetch_options),
        None,
    )?;
    println!(); // New line after progress

    // Merge or fast-forward if possible
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    if analysis.0.is_up_to_date() {
        Ok(())
    } else if analysis.0.is_fast_forward() {
        let refname = "refs/heads/main";
        let mut reference = repo
            .find_reference(refname)
            .or_else(|_| repo.find_reference("refs/heads/master"))?;
        reference.set_target(fetch_commit.id(), "Fast-Forward")?;
        repo.set_head(refname)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        Ok(())
    } else {
        // Could not fast-forward, might need manual merge
        Ok(())
    }
}

/// Checkout the default branch (main or master)
fn checkout_default_branch(repo: &Repository) -> Result<()> {
    // Try main first, then master
    let branch_name = if repo.find_reference("refs/heads/main").is_ok() {
        "refs/heads/main"
    } else {
        "refs/heads/master"
    };

    let obj = repo.revparse_single(branch_name)?;
    repo.checkout_tree(&obj, None)?;
    repo.set_head(branch_name)?;

    Ok(())
}

/// Parse all tapplet configurations from a repository
fn parse_tapplets_from_repo(repo_path: &Path) -> Result<Vec<TappletConfig>> {
    let mut tapplets = Vec::new();

    // Walk through the repository looking for .toml files
    for entry in walkdir::WalkDir::new(repo_path.join("tapplets"))
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip .git directory
        if path.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }

        // Look for tapplet.toml or files ending in -tapplet.toml
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str())
            && file_name == "manifest.toml"
        {
            match TappletConfig::from_file(path.to_str().unwrap()) {
                Ok(config) => tapplets.push(config),
                Err(e) => {
                    eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(tapplets)
}

/// Sanitize a repository URL to create a safe directory name
fn sanitize_repo_name(url: &str) -> String {
    // Remove protocol prefix
    let without_protocol = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("git://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .unwrap_or(url);

    // Remove .git suffix
    let without_suffix = without_protocol
        .strip_suffix(".git")
        .unwrap_or(without_protocol);

    // Replace invalid characters with underscores
    without_suffix
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
