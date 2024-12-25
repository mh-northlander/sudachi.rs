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

pub mod anchor;
pub mod builder;
pub mod error;
pub mod projection;
pub mod resolver;
pub mod source;

use std::env::current_exe;
use std::path::{Path, PathBuf};

use lazy_static::lazy_static;
use serde_json::Value;

pub use anchor::PathAnchor;
pub use builder::ConfigBuilder;
pub use error::ConfigError;
pub use projection::SurfaceProjection;
pub use source::DataSource;

#[deprecated(
    since = "0.7.0",
    note = "default resources are now embedded in the binary"
)]
const DEFAULT_RESOURCE_DIR: &str = "resources";

const DEFAULT_SETTING_FILE: &str = "sudachi.json";
const DEFAULT_DICT_FILE: &str = "system_core.dic";
pub(crate) const DEFAULT_CHAR_DEF_FILE: &str = "char.def";
pub(crate) const DEFAULT_REWRITE_DEF_FILE: &str = "rewrite.def";
pub(crate) const DEFAULT_UNK_DEF_FILE: &str = "unk.def";

/// Setting data loaded from config file
#[derive(Debug, Default, Clone)]
pub struct Config {
    /// Paths will be resolved against this anchor, until a data source will be found
    pub anchor: PathAnchor,

    pub system_dict: PathBuf,
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
    #[deprecated(
        since = "0.7.0",
        note = "user should use proper PathAnchor to control the resource file resoltion"
    )]
    pub fn new(
        config_file: Option<PathBuf>,
        resource_dir: Option<PathBuf>,
        dictionary_path: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        // prioritize arg (cli option) > default
        let mut builder = match config_file {
            Some(p) => ConfigBuilder::from_file(p)?,
            None => ConfigBuilder::from_embedded()?,
        };

        // prioritize arg (cli option) > config file
        if let Some(p) = resource_dir {
            let mut anchor = PathAnchor::new_filesystem(p);
            anchor.append(&mut builder.anchor);
            builder = builder.with_anchor(anchor);
        }

        // prioritize arg (cli option) > config file
        if let Some(p) = dictionary_path {
            builder = builder.system_dict(p);
        }

        Ok(builder.build())
    }

    /// Creates a default config (with a default path anchor)
    pub fn new_embedded() -> Result<Self, ConfigError> {
        let builder = ConfigBuilder::from_embedded()?;
        Ok(builder.build())
    }

    /// Creates a minimal config with the provided resource directory
    pub fn minimal_at(resource_dir: impl Into<PathBuf>) -> Self {
        Config {
            anchor: PathAnchor::new_filesystem(resource_dir.into()),
            system_dict: DEFAULT_DICT_FILE.into(),
            character_definition_file: DEFAULT_CHAR_DEF_FILE.into(),
            oov_provider_plugins: vec![serde_json::json!(
                { "class" : "com.worksap.nlp.sudachi.SimpleOovPlugin",
                  "oovPOS" : [ "名詞", "普通名詞", "一般", "*", "*", "*" ],
                  "leftId" : 0,
                  "rightId" : 0,
                  "cost" : 30000 }
            )],
            ..Default::default()
        }
    }

    /// Sets the system dictionary to the provided path
    pub fn with_system_dic(mut self, system: impl Into<PathBuf>) -> Self {
        self.system_dict = system.into();
        self
    }

    /// resolve path in filesystem
    pub fn resolve_paths(&self, mut path: String) -> Vec<String> {
        // resolve "$exe/" as the parent directory of the current executable
        if path.starts_with("$exe") {
            path.replace_range(0..4, &CURRENT_EXE_DIR);

            let mut path2 = path.clone();
            path2.insert_str(CURRENT_EXE_DIR.len(), "/deps");
            return vec![path2, path];
        }

        // resolve "$cfg/" using (filesystem) path anchor
        if path.starts_with("$cfg/") || path.starts_with("$cfg\\") {
            let roots = self.anchor.filesystem_roots();
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
    #[deprecated(
        since = "0.7.0",
        note = "User should use `resolve` with proper anchor to control path completion"
    )]
    pub fn complete_path<P: AsRef<Path> + Into<PathBuf>>(
        &self,
        file_path: P,
    ) -> Result<PathBuf, ConfigError> {
        let pref = file_path.as_ref();
        // 1. absolute paths are not normalized
        if pref.is_absolute() {
            return Ok(file_path.into());
        }

        // 2. try to resolve paths wrt (filesystem) anchors
        if let Some(p) = self.anchor.first_existing_path(pref) {
            return Ok(p);
        }

        // 3. try to resolve path wrt CWD
        if pref.exists() {
            return Ok(file_path.into());
        }

        // Report an error
        Err(self.anchor.resolution_failure(&file_path))
    }

    /// resolve path as DataSouce wrt the anchor
    pub fn resolve<P: AsRef<Path>>(&self, path: P) -> Result<DataSource, ConfigError> {
        self.anchor.resolve(path)
    }

    /// resolve system dictionary as data source
    pub fn resolved_system_dict(&self) -> Result<DataSource, ConfigError> {
        self.resolve::<&Path>(self.system_dict.as_ref())
    }

    /// resolve user dictionary as list of data sources
    pub fn resolved_user_dicts(&self) -> Result<Vec<DataSource>, ConfigError> {
        self.user_dicts.iter().map(|p| self.resolve(p)).collect()
    }

    /// resolve character definition as data source
    pub fn resolved_char_category(&self) -> Result<DataSource, ConfigError> {
        self.resolve::<&Path>(self.character_definition_file.as_ref())
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
    use super::projection::SurfaceProjection;
    use super::CURRENT_EXE_DIR;
    use super::*;
    use crate::prelude::SudachiResult;

    #[test]
    fn resolve_exe() -> SudachiResult<()> {
        let cfg = Config::new_embedded()?;
        let npath = cfg.resolve_paths("$exe/data".to_owned());
        let exe_dir: &str = &CURRENT_EXE_DIR;
        assert_eq!(npath.len(), 2);
        assert!(npath[0].starts_with(exe_dir));
        Ok(())
    }

    #[test]
    fn resolve_cfg() -> SudachiResult<()> {
        let cfg = Config::new_embedded()?;
        let npath = cfg.resolve_paths("$cfg/data".to_owned());
        let path_dir: &str = "data";
        assert_eq!(1, npath.len());
        assert!(npath[0] == path_dir);
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
