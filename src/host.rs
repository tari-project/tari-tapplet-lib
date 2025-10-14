use crate::model::TappletConfig;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use tokio::{runtime::Handle, task};
use wasmer::{Instance, Module, Store, Value as WasmValue};

#[cfg(feature = "host")]
use mlua::{Lua, MultiValue, Table};

#[derive(Debug)]
pub enum HostError {
    WasmLoadError(String),
    WasmCompileError(String),
    WasmInstantiationError(String),
    LuaLoadError(String),
    LuaExecutionError(String),
    MethodNotFound(String),
    ExecutionError(String),
    InvalidArguments(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::WasmLoadError(msg) => write!(f, "WASM load error: {}", msg),
            HostError::WasmCompileError(msg) => write!(f, "WASM compile error: {}", msg),
            HostError::WasmInstantiationError(msg) => {
                write!(f, "WASM instantiation error: {}", msg)
            }
            HostError::LuaLoadError(msg) => write!(f, "Lua load error: {}", msg),
            HostError::LuaExecutionError(msg) => write!(f, "Lua execution error: {}", msg),
            HostError::MethodNotFound(method) => write!(f, "Method not found: {}", method),
            HostError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            HostError::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            HostError::IoError(err) => write!(f, "IO error: {}", err),
        }
    }
}

impl std::error::Error for HostError {}

impl From<std::io::Error> for HostError {
    fn from(err: std::io::Error) -> Self {
        HostError::IoError(err)
    }
}

impl From<wasmer::CompileError> for HostError {
    fn from(err: wasmer::CompileError) -> Self {
        HostError::WasmCompileError(err.to_string())
    }
}

impl From<wasmer::InstantiationError> for HostError {
    fn from(err: wasmer::InstantiationError) -> Self {
        HostError::WasmInstantiationError(err.to_string())
    }
}

impl From<wasmer::RuntimeError> for HostError {
    fn from(err: wasmer::RuntimeError) -> Self {
        HostError::ExecutionError(err.to_string())
    }
}

#[cfg(feature = "host")]
impl From<mlua::Error> for HostError {
    fn from(err: mlua::Error) -> Self {
        HostError::LuaExecutionError(err.to_string())
    }
}

pub struct WasmTappletHost {
    config: TappletConfig,
    store: Store,
    instance: Instance,
}

impl WasmTappletHost {
    /// Create a new TappletHost by loading a WASM module from a file
    pub fn new(config: TappletConfig, wasm_path: impl AsRef<Path>) -> Result<Self, HostError> {
        // Read the WASM file
        let wasm_bytes = std::fs::read(wasm_path)?;

        // Create a new store
        let mut store = Store::default();

        // Compile the WASM module
        let module = Module::new(&store, wasm_bytes)?;

        // Instantiate the module
        let instance = Instance::new(&mut store, &module, &wasmer::imports! {})?;

        Ok(Self {
            config,
            store,
            instance,
        })
    }

    /// Create a new TappletHost from WASM bytes
    pub fn from_bytes(config: TappletConfig, wasm_bytes: &[u8]) -> Result<Self, HostError> {
        // Create a new store
        let mut store = Store::default();

        // Compile the WASM module
        let module = Module::new(&store, wasm_bytes)?;

        // Instantiate the module
        let instance = Instance::new(&mut store, &module, &wasmer::imports! {})?;

        Ok(Self {
            config,
            store,
            instance,
        })
    }

    /// Run a method with the given arguments
    ///
    /// # Arguments
    /// * `method` - The name of the method to call
    /// * `args` - JSON value representing the arguments
    ///
    /// # Returns
    /// A JSON value containing the result of the method call
    pub fn run(&mut self, method: &str, args: Value) -> Result<Value, HostError> {
        // Verify the method exists in the API config
        if !self.config.api.methods.contains(&method.to_string()) {
            return Err(HostError::MethodNotFound(method.to_string()));
        }

        // Get the exported function from the WASM instance
        let func = self
            .instance
            .exports
            .get_function(method)
            .map_err(|_| HostError::MethodNotFound(method.to_string()))?;

        // Convert JSON args to WASM values
        let wasm_args = self.json_to_wasm_args(&args)?;

        // Call the function
        let results = func
            .call(&mut self.store, &wasm_args)
            .map_err(|e| HostError::ExecutionError(e.to_string()))?;

        // Convert results back to JSON
        let result = self.wasm_results_to_json(&results)?;

        Ok(result)
    }

    /// Convert JSON arguments to WASM values
    fn json_to_wasm_args(&self, args: &Value) -> Result<Vec<WasmValue>, HostError> {
        let mut wasm_args = Vec::new();

        match args {
            Value::Array(arr) => {
                for arg in arr {
                    wasm_args.push(self.json_value_to_wasm(arg)?);
                }
            }
            Value::Object(obj) => {
                // For object arguments, convert each value
                for (_key, value) in obj {
                    wasm_args.push(self.json_value_to_wasm(value)?);
                }
            }
            _ => {
                // Single argument
                wasm_args.push(self.json_value_to_wasm(args)?);
            }
        }

        Ok(wasm_args)
    }

    /// Convert a single JSON value to a WASM value
    fn json_value_to_wasm(&self, value: &Value) -> Result<WasmValue, HostError> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(WasmValue::I64(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(WasmValue::F64(f))
                } else {
                    Err(HostError::InvalidArguments(format!(
                        "Unsupported number type: {}",
                        n
                    )))
                }
            }
            Value::Bool(b) => Ok(WasmValue::I32(if *b { 1 } else { 0 })),
            Value::String(_s) => {
                // For strings, we'd typically need to pass a pointer and length
                // This is a simplified version - in practice you'd need memory management
                Err(HostError::InvalidArguments(
                    "String arguments require memory management - not yet implemented".to_string(),
                ))
            }
            _ => Err(HostError::InvalidArguments(format!(
                "Unsupported argument type: {:?}",
                value
            ))),
        }
    }

    /// Convert WASM results to JSON
    fn wasm_results_to_json(&self, results: &[WasmValue]) -> Result<Value, HostError> {
        if results.is_empty() {
            return Ok(Value::Null);
        }

        if results.len() == 1 {
            return self.wasm_value_to_json(&results[0]);
        }

        // Multiple results - return as array
        let mut json_results = Vec::new();
        for result in results {
            json_results.push(self.wasm_value_to_json(result)?);
        }

        Ok(Value::Array(json_results))
    }

    /// Convert a single WASM value to JSON
    fn wasm_value_to_json(&self, value: &WasmValue) -> Result<Value, HostError> {
        match value {
            WasmValue::I32(i) => Ok(Value::Number((*i).into())),
            WasmValue::I64(i) => Ok(Value::Number((*i).into())),
            WasmValue::F32(f) => {
                if let Some(n) = serde_json::Number::from_f64(*f as f64) {
                    Ok(Value::Number(n))
                } else {
                    Err(HostError::ExecutionError(
                        "Failed to convert F32 to JSON number".to_string(),
                    ))
                }
            }
            WasmValue::F64(f) => {
                if let Some(n) = serde_json::Number::from_f64(*f) {
                    Ok(Value::Number(n))
                } else {
                    Err(HostError::ExecutionError(
                        "Failed to convert F64 to JSON number".to_string(),
                    ))
                }
            }
            _ => Err(HostError::ExecutionError(format!(
                "Unsupported WASM value type: {:?}",
                value
            ))),
        }
    }

    /// Get the tapplet configuration
    pub fn config(&self) -> &TappletConfig {
        &self.config
    }
}

/// Convenience function to run a method on a tapplet
///
/// # Arguments
/// * `config` - The tapplet configuration
/// * `wasm_path` - Path to the WASM file
/// * `method` - The name of the method to call
/// * `args` - JSON value representing the arguments
///
/// # Returns
/// A JSON value containing the result of the method call
pub fn run(
    config: TappletConfig,
    wasm_path: impl AsRef<Path>,
    method: &str,
    args: Value,
) -> Result<Value, HostError> {
    let mut host = WasmTappletHost::new(config, wasm_path)?;
    host.run(method, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_error_display() {
        let err = HostError::MethodNotFound("test_method".to_string());
        assert_eq!(err.to_string(), "Method not found: test_method");
    }

    #[test]
    fn test_invalid_wasm_error() {
        let config = TappletConfig {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            friendly_name: "Test".to_string(),
            description: "Test tapplet".to_string(),
            publisher: "test_publisher".to_string(),
            git: crate::model::GitConfig {
                url: "https://example.com".to_string(),
                rev: "main".to_string(),
            },
            api: crate::model::ApiConfig {
                methods: vec!["test".to_string()],
                method_definitions: std::collections::HashMap::new(),
            },
            sigs: crate::model::SigsConfig {
                todo: "test".to_string(),
            },
        };

        // Create an invalid WASM module for testing error handling
        let wasm_bytes = vec![0x00, 0x61, 0x73, 0x6d];

        let result = TappletHost::from_bytes(config, &wasm_bytes);
        // This should fail because it's not a complete valid WASM module
        assert!(result.is_err());
        if let Err(e) = result {
            // Verify we get a proper error message
            assert!(!e.to_string().is_empty());
        }
    }
}

#[async_trait]
pub trait MinotariTappletApiV1: Clone {
    async fn append_data(&self, slot: &str, value: &str) -> Result<(), anyhow::Error>;
    async fn load_data_entries(&self, slot: &str) -> Result<Vec<String>, anyhow::Error>;
}

pub struct LuaTappletHost<T> {
    config: TappletConfig,
    lua: Lua,
    api: T,
}

impl<T: MinotariTappletApiV1 + 'static> LuaTappletHost<T> {
    /// Create a new LuaTappletHost by loading a Lua script from a file
    pub fn new(
        config: TappletConfig,
        lua_path: impl AsRef<Path>,
        api: T,
    ) -> Result<Self, HostError> {
        // Read the Lua file
        let lua_code = std::fs::read_to_string(lua_path)?;

        // Create a new Lua instance
        let lua = Lua::new();
        lua.sandbox(true)?;

        // Load and execute the Lua code to define functions
        lua.load(&lua_code)
            .exec()
            .map_err(|e| HostError::LuaLoadError(e.to_string()))?;

        Ok(Self { config, lua, api })
    }

    /// Create a new LuaTappletHost from a Lua code string
    pub fn from_string(config: TappletConfig, lua_code: &str, api: T) -> Result<Self, HostError> {
        // Create a new Lua instance
        let lua = Lua::new();

        // Load and execute the Lua code to define functions
        lua.load(lua_code)
            .exec()
            .map_err(|e| HostError::LuaLoadError(e.to_string()))?;

        Ok(Self { config, lua, api })
    }

    /// Run a method with the given arguments
    ///
    /// # Arguments
    /// * `method` - The name of the method to call
    /// * `args` - JSON value representing the arguments
    ///
    /// # Returns
    /// A JSON value containing the result of the method call
    pub async fn run(&self, method: &str, args: Value) -> Result<Value, HostError> {
        // Verify the method exists in the API config
        if !self.config.api.methods.contains(&method.to_string()) {
            return Err(HostError::MethodNotFound(method.to_string()));
        }

        // Get the Lua function
        let func: mlua::Function = self
            .lua
            .globals()
            .get(method)
            .map_err(|_| HostError::MethodNotFound(method.to_string()))?;

        // Convert JSON args to Lua values
        let lua_args = self.json_to_lua_value(&args)?;

        // load API
        let api2 = self.api.clone();

        let rust_append_data =
            self.lua
                .create_function(move |_, (slot, value): (String, String)| {
                    task::block_in_place(|| {
                        Handle::current().block_on(async {
                            api2.append_data(&slot, &value).await?;
                            Result::<_, anyhow::Error>::Ok(())
                        })?;
                        Ok(())
                    })
                })?;
        let api3 = self.api.clone();
        let rust_load_data_entries = self.lua.create_function(move |l, slot: String| {
            task::block_in_place(|| {
                let result = Handle::current().block_on(async {
                    let table = l.create_table()?;
                    // println!("Loading data entries from slot '{}'", slot);
                    let entries = api3.load_data_entries(&slot).await?;
                    for (i, entry) in entries.iter().enumerate() {
                        table.set(i + 1, entry.clone())?;
                    }
                    Result::<_, anyhow::Error>::Ok(entries)
                })?;
                Ok(result)
            })
        })?;

        self.lua
            .globals()
            .set("minotari_append_data", rust_append_data)?;
        self.lua
            .globals()
            .set("minotari_load_data_entries", rust_load_data_entries)?;

        // self.lua.globals().set("api", self.lua.create_table()?)?;

        // Call the function
        let result: mlua::Value = func
            .call(lua_args)
            .map_err(|e| HostError::LuaExecutionError(e.to_string()))?;

        // Convert result back to JSON
        let json_result = self.lua_value_to_json(&result)?;

        Ok(json_result)
    }

    /// Convert JSON value to Lua value
    fn json_to_lua_value(&self, value: &Value) -> Result<mlua::Value, HostError> {
        match value {
            Value::Null => Ok(mlua::Value::Nil),
            Value::Bool(b) => Ok(mlua::Value::Boolean(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                        return Ok(mlua::Value::Integer(i as i32));
                    } else {
                        return Err(HostError::InvalidArguments(format!(
                            "Integer out of range for Lua: {}",
                            i
                        )));
                    }
                } else if let Some(f) = n.as_f64() {
                    Ok(mlua::Value::Number(f))
                } else {
                    Err(HostError::InvalidArguments(format!(
                        "Unsupported number type: {}",
                        n
                    )))
                }
            }
            Value::String(s) => self
                .lua
                .create_string(s)
                .map(mlua::Value::String)
                .map_err(|e| HostError::InvalidArguments(e.to_string())),
            Value::Array(arr) => {
                let table = self.lua.create_table().map_err(|e| {
                    HostError::InvalidArguments(format!("Failed to create table: {}", e))
                })?;
                for (i, item) in arr.iter().enumerate() {
                    let lua_value = self.json_to_lua_value(item)?;
                    table
                        .set(i + 1, lua_value)
                        .map_err(|e| HostError::InvalidArguments(e.to_string()))?;
                }
                Ok(mlua::Value::Table(table))
            }
            Value::Object(obj) => {
                let table = self.lua.create_table().map_err(|e| {
                    HostError::InvalidArguments(format!("Failed to create table: {}", e))
                })?;
                for (key, val) in obj {
                    let lua_value = self.json_to_lua_value(val)?;
                    table
                        .set(key.as_str(), lua_value)
                        .map_err(|e| HostError::InvalidArguments(e.to_string()))?;
                }
                Ok(mlua::Value::Table(table))
            }
        }
    }

    /// Convert Lua value to JSON value
    fn lua_value_to_json(&self, value: &mlua::Value) -> Result<Value, HostError> {
        match value {
            mlua::Value::Nil => Ok(Value::Null),
            mlua::Value::Boolean(b) => Ok(Value::Bool(*b)),
            mlua::Value::Integer(i) => Ok(Value::Number((*i).into())),
            mlua::Value::Number(n) => {
                if let Some(num) = serde_json::Number::from_f64(*n) {
                    Ok(Value::Number(num))
                } else {
                    Err(HostError::ExecutionError(
                        "Failed to convert Lua number to JSON".to_string(),
                    ))
                }
            }
            mlua::Value::String(s) => {
                let str_val = s
                    .to_str()
                    .map_err(|e| HostError::ExecutionError(e.to_string()))?;
                Ok(Value::String(str_val.to_string()))
            }
            mlua::Value::Table(table) => {
                // Check if it's an array (sequential integer keys starting from 1)
                let len = table
                    .len()
                    .map_err(|e| HostError::ExecutionError(e.to_string()))?;

                if len > 0 {
                    // Try to treat as array
                    let mut arr = Vec::new();
                    for i in 1..=len {
                        let val: mlua::Value = table
                            .get(i)
                            .map_err(|e| HostError::ExecutionError(e.to_string()))?;
                        arr.push(self.lua_value_to_json(&val)?);
                    }
                    Ok(Value::Array(arr))
                } else {
                    // Treat as object
                    let mut obj = serde_json::Map::new();
                    for pair in table.pairs::<mlua::Value, mlua::Value>() {
                        let (key, val) =
                            pair.map_err(|e| HostError::ExecutionError(e.to_string()))?;

                        // Convert key to string
                        let key_str = match key {
                            mlua::Value::String(s) => s
                                .to_str()
                                .map_err(|e| HostError::ExecutionError(e.to_string()))?
                                .to_string(),
                            mlua::Value::Integer(i) => i.to_string(),
                            mlua::Value::Number(n) => n.to_string(),
                            _ => {
                                return Err(HostError::ExecutionError(
                                    "Unsupported table key type".to_string(),
                                ));
                            }
                        };

                        obj.insert(key_str, self.lua_value_to_json(&val)?);
                    }
                    Ok(Value::Object(obj))
                }
            }
            _ => Err(HostError::ExecutionError(format!(
                "Unsupported Lua value type: {:?}",
                value
            ))),
        }
    }

    /// Get the tapplet configuration
    pub fn config(&self) -> &TappletConfig {
        &self.config
    }
}
