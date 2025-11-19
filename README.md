# tari-tapplet-lib

A Rust library for managing and executing tapplets (small extensible plugins) in the Tari ecosystem. Tapplets are modular components that can be implemented in WebAssembly, Lua, or other formats, with a standardized configuration and execution model.

## Features

- **Multi-Language Support**: Execute tapplets written in WebAssembly (WASM), Lua scripts, or native Rust binaries NOTE: WASM is unstable and under development
- **Configuration Management**: Parse and manage tapplet metadata via TOML manifest files
- **Registry System**: Manage collections of tapplets with Git-based repository cloning and caching
- **Installation Management**: Support for WASM and Lua tapplet installation with automatic compilation and caching
- **Sandboxed Execution**: Secure execution environment with Lua sandboxing

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tari-tapplet-lib = "0.1.0"
```

To enable WASM and Lua execution capabilities:

```toml
[dependencies]
tari-tapplet-lib = { version = "0.1.0", features = ["host"] }
```

## Usage

### Parsing a Tapplet Configuration

```rust
use tari_tapplet_lib::parse_tapplet_file;

let config = parse_tapplet_file("path/to/manifest.toml")?;
println!("Tapplet: {}", config.name);
```

### Loading and Using a Registry

```rust
use tari_tapplet_lib::TappletRegistry;
use std::path::PathBuf;

let mut registry = TappletRegistry::new(
    "myregistry",
    "https://github.com/example/tapplet-registry",
    PathBuf::from("./cache")
);

// Fetch tapplets from remote
registry.fetch().await?;

// Search for tapplets
let results = registry.search("password")?;
```

### Executing a WASM Tapplet

Requires the `host` feature.

```rust
use tari_tapplet_lib::{TappletConfig, host::WasmTappletHost};
use serde_json::json;

let config = TappletConfig::from_file("manifest.toml")?;
let mut host = WasmTappletHost::new(config, "path/to/tapplet.wasm")?;

let result = host.run("greet", json!(["Alice"]))?;
println!("Result: {}", result);
```

### Executing a Lua Tapplet

Requires the `host` feature.

```rust
use tari_tapplet_lib::{TappletConfig, host::{LuaTappletHost, MinotariTappletApiV1}};
use async_trait::async_trait;

// Implement MinotariTappletApiV1
struct MyApi;

#[async_trait]
impl MinotariTappletApiV1 for MyApi {
    async fn append_data(&self, slot: &str, value: &str) -> Result<(), anyhow::Error> {
        // Your implementation
        Ok(())
    }

    async fn load_data_entries(&self, slot: &str) -> Result<Vec<String>, anyhow::Error> {
        // Your implementation
        Ok(Vec::new())
    }
}

let config = TappletConfig::from_file("manifest.toml")?;
let host = LuaTappletHost::new(config, "path/to/tapplet.lua", MyApi)?;
let result = host.run("my_function", json!({})).await?;
```

### Installing Tapplets

#### Lua Tapplet

```rust
use tari_tapplet_lib::local_folder_lua_tapplet::LocalFolderLuaTapplet;
use std::path::PathBuf;

let tapplet = LocalFolderLuaTapplet::load(PathBuf::from("./my_lua_tapplet"))?;
tapplet.install(PathBuf::from("./cache"))?;
```

#### WASM Tapplet

```rust
use tari_tapplet_lib::local_folder_tapplet::LocalFolderTapplet;
use std::path::PathBuf;

let tapplet = LocalFolderTapplet::load(PathBuf::from("./my_wasm_tapplet"))?;
tapplet.install(PathBuf::from("./cache"))?;
```

## Tapplet Manifest Format

Tapplets are configured using a `manifest.toml` file:

```toml
name = "password_manager"
version = "0.1.0"
friendly_name = "Password Manager"
description = "A simple password manager tapplet."
publisher = "a86b454a33b98f7f4f296a86dcbf08eaa816de5347d5c932b5fed8a95c52d04a"
public_key = "a86b454a33b98f7f4f296a86dcbf08eaa816de5347d5c932b5fed8a95c52d04a"
git = { url = "https://github.com/example/tapplet", rev = "main" }

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
```

## Modules

| Module | Description |
|--------|-------------|
| `model` | Core configuration types (`TappletConfig`, `ApiConfig`, etc.) |
| `registry` | Git-based tapplet registry management |
| `git_tapplet` | Install tapplets from Git repositories |
| `local_folder_tapplet` | Manage and install WASM tapplets from local directories |
| `local_folder_lua_tapplet` | Manage and install Lua tapplets from local directories |
| `host` | WASM and Lua execution hosts (requires `host` feature) |

## Lua API

Lua tapplets have access to the following Minotari API functions:

- `minotari_append_data(slot, value)` - Append data to a slot
- `minotari_load_data_entries(slot)` - Load all entries from a slot

## License

See [LICENSE](LICENSE) for details.
