/*
 * Copyright (c) 2024 Works Applications Co., Ltd.
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

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::resolver::PathResolver;
use super::{
    Config, ConfigError, SurfaceProjection, DEFAULT_CHAR_DEF_FILE, DEFAULT_RESOURCE_DIR,
    DEFAULT_SETTING_FILE,
};

pub fn default_resource_dir() -> PathBuf {
    let mut src_root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if !src_root_path.pop() {
        src_root_path.push("..");
    }
    src_root_path.push(DEFAULT_RESOURCE_DIR);
    src_root_path
}

pub fn default_config_location() -> PathBuf {
    let mut resdir = default_resource_dir();
    resdir.push(DEFAULT_SETTING_FILE);
    resdir
}

macro_rules! merge_cfg_value {
    ($base: ident, $o: ident, $name: tt) => {
        $base.$name = $base.$name.or_else(|| $o.$name.clone())
    };
}

/// Struct corresponds with raw config json file.
/// You must use filed names defined here as json object key.
/// For plugins, refer to each plugin.
#[allow(non_snake_case)]
#[derive(Deserialize, Debug, Clone)]
pub struct ConfigBuilder {
    /// Analogue to Java Implementation path Override    
    pub(crate) path: Option<PathBuf>,
    /// User-passed resourcePath
    #[serde(skip)]
    resourcePath: Option<PathBuf>,
    /// User-passed root directory.
    /// Is also automatically set on from_file
    #[serde(skip)]
    rootDirectory: Option<PathBuf>,
    #[serde(alias = "system")]
    systemDict: Option<PathBuf>,
    #[serde(alias = "user")]
    userDict: Option<Vec<PathBuf>>,
    characterDefinitionFile: Option<PathBuf>,
    connectionCostPlugin: Option<Vec<Value>>,
    inputTextPlugin: Option<Vec<Value>>,
    oovProviderPlugin: Option<Vec<Value>>,
    pathRewritePlugin: Option<Vec<Value>>,
    projection: Option<SurfaceProjection>,
}

impl ConfigBuilder {
    pub fn from_opt_file(config_file: Option<&Path>) -> Result<Self, ConfigError> {
        match config_file {
            None => {
                let default_config = default_config_location();
                Self::from_file(&default_config)
            }
            Some(cfg) => Self::from_file(cfg),
        }
    }

    pub fn from_file(config_file: &Path) -> Result<Self, ConfigError> {
        let file = File::open(config_file)?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .map_err(|e| e.into())
            .map(|cfg: ConfigBuilder| match config_file.parent() {
                Some(p) => cfg.root_directory(p),
                None => cfg,
            })
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, ConfigError> {
        serde_json::from_slice(data).map_err(|e| e.into())
    }

    pub fn empty() -> Self {
        serde_json::from_slice(b"{}").unwrap()
    }

    pub fn system_dict(mut self, dict: impl Into<PathBuf>) -> Self {
        self.systemDict = Some(dict.into());
        self
    }

    pub fn user_dict(mut self, dict: impl Into<PathBuf>) -> Self {
        let dicts = match self.userDict.as_mut() {
            None => {
                self.userDict = Some(Default::default());
                self.userDict.as_mut().unwrap()
            }
            Some(dicts) => dicts,
        };
        dicts.push(dict.into());
        self
    }

    pub fn resource_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.resourcePath = Some(path.into());
        self
    }

    pub fn root_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.rootDirectory = Some(path.into());
        self
    }

    pub fn build(self) -> Config {
        let default_resource_dir = default_resource_dir();
        let resource_dir = self.resourcePath.unwrap_or(default_resource_dir);

        let mut resolver = PathResolver::with_capacity(3);
        let mut add_path = |buf: PathBuf| {
            if !resolver.contains(&buf) {
                resolver.add(buf);
            }
        };
        self.path.map(&mut add_path);
        add_path(resource_dir);
        self.rootDirectory.map(&mut add_path);

        let character_definition_file = self
            .characterDefinitionFile
            .unwrap_or(PathBuf::from(DEFAULT_CHAR_DEF_FILE));

        Config {
            resolver,
            system_dict: self.systemDict,
            user_dicts: self.userDict.unwrap_or_default(),
            character_definition_file,

            connection_cost_plugins: self.connectionCostPlugin.unwrap_or_default(),
            input_text_plugins: self.inputTextPlugin.unwrap_or_default(),
            oov_provider_plugins: self.oovProviderPlugin.unwrap_or_default(),
            path_rewrite_plugins: self.pathRewritePlugin.unwrap_or_default(),
            projection: self.projection.unwrap_or(SurfaceProjection::Surface),
        }
    }

    pub fn fallback(mut self, other: &ConfigBuilder) -> ConfigBuilder {
        merge_cfg_value!(self, other, path);
        merge_cfg_value!(self, other, resourcePath);
        merge_cfg_value!(self, other, rootDirectory);
        merge_cfg_value!(self, other, systemDict);
        merge_cfg_value!(self, other, userDict);
        merge_cfg_value!(self, other, characterDefinitionFile);
        merge_cfg_value!(self, other, connectionCostPlugin);
        merge_cfg_value!(self, other, inputTextPlugin);
        merge_cfg_value!(self, other, oovProviderPlugin);
        merge_cfg_value!(self, other, pathRewritePlugin);
        merge_cfg_value!(self, other, projection);
        self
    }
}
