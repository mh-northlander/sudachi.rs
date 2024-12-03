/*
 * Copyright (c) 2021-2024 Works Applications Co., Ltd.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

pub mod builder;
pub mod error;
pub mod projection;
pub mod resolver;

use std::env::current_exe;
use std::path::{Path, PathBuf};

use lazy_static::lazy_static;
use serde_json::Value;

pub use builder::ConfigBuilder;
pub use error::ConfigError;
pub use projection::SurfaceProjection;
use resolver::PathResolver;

const DEFAULT_RESOURCE_DIR: &str = "resources";
const DEFAULT_SETTING_FILE: &str = "sudachi.json";
const DEFAULT_SETTING_BYTES: &[u8] = include_bytes!("../../resources/sudachi.json");
const DEFAULT_CHAR_DEF_FILE: &str = "char.def";

/// Setting data loaded from config file
#[derive(Debug, Default, Clone)]
pub struct Config {
    /// Paths will be resolved against these roots, until a file will be found
    pub(crate) resolver: PathResolver,
    pub system_dict: Option<PathBuf>,
    pub user_dicts: Vec<PathBuf>,
    pub character_definition_file: PathBuf,

    pub connection_cost_plugins: Vec<Value>,
    pub input_text_plugins: Vec<Value>,
    pub oov_provider_plugins: Vec<Value>,
    pub path_rewrite_plugins: Vec<Value>,
    // this option is Python-only and is ignored in Rust APIs
    pub projection: SurfaceProjection,
}

impl Config {
    pub fn new(
        config_file: Option<PathBuf>,
        resource_dir: Option<PathBuf>,
        dictionary_path: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        // prioritize arg (cli option) > default
        let raw_config = ConfigBuilder::from_opt_file(config_file.as_deref())?;

        // prioritize arg (cli option) > config file
        let raw_config = match resource_dir {
            None => raw_config,
            Some(p) => raw_config.resource_path(p),
        };

        // prioritize arg (cli option) > config file
        let raw_config = match dictionary_path {
            None => raw_config,
            Some(p) => raw_config.system_dict(p),
        };

        Ok(raw_config.build())
    }

    pub fn new_embedded() -> Result<Self, ConfigError> {
        let raw_config = ConfigBuilder::from_bytes(DEFAULT_SETTING_BYTES)?;

        Ok(raw_config.build())
    }

    /// Creates a minimal config with the provided resource directory
    pub fn minimal_at(resource_dir: impl Into<PathBuf>) -> Config {
        let mut cfg = Config::default();
        let resource = resource_dir.into();
        cfg.character_definition_file = resource.join(DEFAULT_CHAR_DEF_FILE);
        let mut resolver = PathResolver::with_capacity(1);
        resolver.add(resource);
        cfg.resolver = resolver;
        cfg.oov_provider_plugins = vec![serde_json::json!(
            { "class" : "com.worksap.nlp.sudachi.SimpleOovPlugin",
              "oovPOS" : [ "名詞", "普通名詞", "一般", "*", "*", "*" ],
              "leftId" : 0,
              "rightId" : 0,
              "cost" : 30000 }
        )];
        cfg
    }

    /// Sets the system dictionary to the provided path
    pub fn with_system_dic(mut self, system: impl Into<PathBuf>) -> Config {
        self.system_dict = Some(system.into());
        self
    }

    pub fn resolve_paths(&self, mut path: String) -> Vec<String> {
        if path.starts_with("$exe") {
            path.replace_range(0..4, &CURRENT_EXE_DIR);

            let mut path2 = path.clone();
            path2.insert_str(CURRENT_EXE_DIR.len(), "/deps");
            return vec![path2, path];
        }

        if path.starts_with("$cfg/") || path.starts_with("$cfg\\") {
            let roots = self.resolver.roots();
            let mut result = Vec::with_capacity(roots.len());
            path.replace_range(0..5, "");
            for root in roots {
                let subpath = root.join(&path);
                result.push(subpath.to_string_lossy().into_owned());
            }
            return result;
        }

        vec![path]
    }

    /// Resolves a possibly relative path with regards to all possible anchors:
    /// 1. Absolute paths stay as they are
    /// 2. Paths are resolved wrt to anchors, returning the first existing one
    /// 3. Path are checked wrt to CWD
    /// 4. If all fail, return an error with all candidate paths listed
    pub fn complete_path<P: AsRef<Path> + Into<PathBuf>>(
        &self,
        file_path: P,
    ) -> Result<PathBuf, ConfigError> {
        let pref = file_path.as_ref();
        // 1. absolute paths are not normalized
        if pref.is_absolute() {
            return Ok(file_path.into());
        }

        // 2. try to resolve paths wrt anchors
        if let Some(p) = self.resolver.first_existing(pref) {
            return Ok(p);
        }

        // 3. try to resolve path wrt CWD
        if pref.exists() {
            return Ok(file_path.into());
        }

        // Report an error
        Err(self.resolver.resolution_failure(&file_path))
    }

    pub fn resolved_system_dict(&self) -> Result<PathBuf, ConfigError> {
        match self.system_dict.as_ref() {
            Some(p) => self.complete_path(p),
            None => Err(ConfigError::MissingArgument("systemDict".to_owned())),
        }
    }

    pub fn resolved_user_dicts(&self) -> Result<Vec<PathBuf>, ConfigError> {
        self.user_dicts
            .iter()
            .map(|p| self.complete_path(p))
            .collect()
    }
}

fn current_exe_dir() -> String {
    let exe = current_exe().unwrap_or_else(|e| panic!("Current exe is not available {:?}", e));

    let parent = exe
        .parent()
        .unwrap_or_else(|| panic!("Path to executable must have a parent"));

    parent.to_str().map(|s| s.to_owned()).unwrap_or_else(|| {
        panic!("placing Sudachi in directories with non-utf paths is not supported")
    })
}

lazy_static! {
    pub(crate) static ref CURRENT_EXE_DIR: String = current_exe_dir();
}

#[cfg(test)]
mod tests {
    use super::builder::default_resource_dir;
    use super::CURRENT_EXE_DIR;
    use super::projection::SurfaceProjection;
    use super::*;
    use crate::prelude::SudachiResult;

    #[test]
    fn resolve_exe() -> SudachiResult<()> {
        let cfg = Config::new(None, None, None)?;
        let npath = cfg.resolve_paths("$exe/data".to_owned());
        let exe_dir: &str = &CURRENT_EXE_DIR;
        assert_eq!(npath.len(), 2);
        assert!(npath[0].starts_with(exe_dir));
        Ok(())
    }

    #[test]
    fn resolve_cfg() -> SudachiResult<()> {
        let cfg = Config::new(None, None, None)?;
        let npath = cfg.resolve_paths("$cfg/data".to_owned());
        let def = default_resource_dir();
        let path_dir: &str = def.to_str().unwrap();
        assert_eq!(1, npath.len());
        assert!(npath[0].starts_with(path_dir));
        Ok(())
    }

    #[test]
    fn config_builder_fallback() {
        let mut cfg = ConfigBuilder::empty();
        cfg.path = Some("test".into());
        let cfg2 = ConfigBuilder::empty();
        let cfg2 = cfg2.fallback(&cfg);
        assert_eq!(cfg2.path, Some("test".into()));
    }

    #[test]
    fn surface_projection_tryfrom() {
        assert_eq!(
            SurfaceProjection::Surface,
            SurfaceProjection::try_from("surface").unwrap()
        );
    }
}
