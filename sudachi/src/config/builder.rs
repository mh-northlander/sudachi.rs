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

#[allow(deprecated)]
use super::DEFAULT_RESOURCE_DIR;
use super::{
    Config, ConfigError, DataSource, PathAnchor, SurfaceProjection, DEFAULT_CHAR_DEF_FILE,
    DEFAULT_DICT_FILE, DEFAULT_SETTING_FILE,
};

#[allow(dead_code, deprecated)]
#[deprecated(
    since = "0.7.0",
    note = "default resources are now embedded in the binary"
)]
pub fn default_resource_dir() -> PathBuf {
    let mut src_root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if !src_root_path.pop() {
        src_root_path.push("..");
    }
    src_root_path.push(DEFAULT_RESOURCE_DIR);
    src_root_path
}

#[allow(dead_code, deprecated)]
#[deprecated(since = "0.7.0", note = "default config is now embedded in the binary")]
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
    // anchor
    #[serde(skip)]
    pub anchor: PathAnchor,

    /// Analogue to Java Implementation path Override
    pub(crate) path: Option<PathBuf>,

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
    /// empty builder with empty anchor
    pub fn empty() -> Self {
        serde_json::from_slice(b"{}").unwrap()
    }

    /// load config from the embedded resouces
    pub fn from_embedded() -> Result<Self, ConfigError> {
        Self::from_anchor(PathAnchor::new_default())
    }

    /// load default config file from the anchor
    pub fn from_anchor(anchor: PathAnchor) -> Result<Self, ConfigError> {
        Self::from_anchored_file(PathBuf::from(DEFAULT_SETTING_FILE), anchor)
    }

    /// load config json file from the anchor
    pub fn from_anchored_file<P: AsRef<Path>>(
        config_file: P,
        anchor: PathAnchor,
    ) -> Result<Self, ConfigError> {
        Self::from_source(anchor.resolve(config_file)?).map(|cfg: Self| cfg.with_anchor(anchor))
    }

    /// load config from file or embedded one
    pub fn from_opt_file<P: AsRef<Path>>(config_file: Option<P>) -> Result<Self, ConfigError> {
        match config_file {
            None => Self::from_embedded(),
            Some(cfg) => Self::from_file(cfg),
        }
    }

    /// load config json file. set default anchor with its parent directory
    pub fn from_file<P: AsRef<Path>>(config_file: P) -> Result<Self, ConfigError> {
        let mut anchor = match config_file.as_ref().parent() {
            Some(p) => PathAnchor::new_filesystem(p),
            None => PathAnchor::empty(),
        };
        anchor.append(&mut PathAnchor::new_default());
        Self::from_anchored_file(config_file, anchor)
    }

    /// load config json from a DataSource. anchor should be set by the caller.
    fn from_source(source: DataSource) -> Result<Self, ConfigError> {
        match source {
            DataSource::File(p) => {
                let file = File::open(p)?;
                let reader = BufReader::new(file);
                serde_json::from_reader(reader)
            }
            DataSource::Borrowed(b) => serde_json::from_slice(b),
            DataSource::Owned(v) => serde_json::from_slice(&v),
        }
        .map_err(|e| e.into())
    }

    /// Read config json from bytes with CWD anchor.
    pub fn from_bytes(data: &[u8]) -> Result<Self, ConfigError> {
        Self::from_bytes_and_anchor(data, PathAnchor::new_cwd())
    }

    /// Read config json from bytes and set provided anchor
    pub fn from_bytes_and_anchor(data: &[u8], anchor: PathAnchor) -> Result<Self, ConfigError> {
        serde_json::from_slice(data)
            .map_err(|e| e.into())
            .map(|cfg: Self| cfg.with_anchor(anchor))
    }

    /// Sets the anchor to the provided one
    pub fn with_anchor(mut self, anchor: PathAnchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Append provided anchor to the current one
    pub fn append_anchor(mut self, anchor: &mut PathAnchor) -> Self {
        self.anchor.append(anchor);
        self
    }

    /// Set system dict path
    pub fn system_dict(mut self, dict: impl Into<PathBuf>) -> Self {
        self.systemDict = Some(dict.into());
        self
    }

    /// Push user dict path
    #[deprecated(since = "0.7.0", note = "use add_user_dict instead")]
    pub fn user_dict(self, dict: impl Into<PathBuf>) -> Self {
        self.add_user_dict(dict)
    }

    /// Push user dict path
    pub fn add_user_dict(mut self, dict: impl Into<PathBuf>) -> Self {
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

    /// Clear the current user dict path list
    pub fn clear_user_dict(mut self) -> Self {
        self.userDict = None;
        self
    }

    /// Bulid a Config from this builder.
    pub fn build(self) -> Config {
        // prepend path in the config json
        let anchor = match self.path {
            Some(p) => {
                let mut anchor = PathAnchor::new_filesystem(p);
                anchor.append(&mut self.anchor.clone());
                anchor
            }
            None => self.anchor.clone(),
        };

        Config {
            anchor,
            system_dict: self.systemDict.unwrap_or(DEFAULT_DICT_FILE.into()),
            user_dicts: self.userDict.unwrap_or_default(),
            character_definition_file: self
                .characterDefinitionFile
                .unwrap_or(DEFAULT_CHAR_DEF_FILE.into()),
            connection_cost_plugins: self.connectionCostPlugin.unwrap_or_default(),
            input_text_plugins: self.inputTextPlugin.unwrap_or_default(),
            oov_provider_plugins: self.oovProviderPlugin.unwrap_or_default(),
            path_rewrite_plugins: self.pathRewritePlugin.unwrap_or_default(),
            projection: self.projection.unwrap_or(SurfaceProjection::Surface),
        }
    }

    /// Merge another builder to the current one
    pub fn fallback(mut self, other: &ConfigBuilder) -> ConfigBuilder {
        self.anchor.append(&mut other.anchor.clone());
        merge_cfg_value!(self, other, path);
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
