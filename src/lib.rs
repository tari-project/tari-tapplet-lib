pub mod model;

#[cfg(feature = "host")]
pub mod host;

pub mod git_tapplet;
pub mod local_folder_lua_tapplet;
pub mod local_folder_tapplet;
pub mod registry;

use std::path::Path;

pub use model::TappletConfig;
pub use registry::TappletRegistry;

#[cfg(feature = "host")]
pub use host::{HostError, LuaTappletHost, WasmTappletHost, run};

use anyhow::Result;

/// Example usage of parsing a tapplet configuration
pub fn parse_tapplet_file<P: AsRef<Path>>(path: P) -> Result<TappletConfig> {
    TappletConfig::from_file(path)
}
