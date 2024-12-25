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

use std::fmt::Debug;
use std::path::{Path, PathBuf};

use super::ConfigError;
use super::DataSource;
use super::{
    DEFAULT_CHAR_DEF_FILE, DEFAULT_REWRITE_DEF_FILE, DEFAULT_SETTING_FILE, DEFAULT_UNK_DEF_FILE,
};
use crate::dic::character_category::DEFAULT_CHAR_DEF_BYTES;

const DEFAULT_REWRITE_DEF_BYTES: &[u8] = include_bytes!("../../../resources/rewrite.def");
const DEFAULT_UNK_DEF_BYTES: &[u8] = include_bytes!("../../../resources/unk.def");
const DEFAULT_SETTING_BYTES: &[u8] = include_bytes!("../../../resources/sudachi.json");

// trait for components of PathAnchor
pub trait PathResolver: DynClone + Debug + Send + Sync {
    // resolved path base on this resolver
    // TODO: maybe vec is better
    fn candidate(&self, path: &Path) -> Option<DataSource>;

    // root dirs used for DSO filepath resolution
    fn filesystem_roots(&self) -> Option<&PathBuf> {
        None
    }
}

// trait for making PathAnchor clonable
pub trait DynClone {
    fn clone_box(&self) -> Box<dyn PathResolver>;
}

impl<T> DynClone for T
where
    T: PathResolver + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn PathResolver> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn PathResolver> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// anchors used to resolve pathes in the config
#[derive(Default, Clone, Debug)]
pub struct PathAnchor {
    anchors: Vec<Box<dyn PathResolver>>,
}

impl PathAnchor {
    pub fn empty() -> Self {
        PathAnchor { anchors: vec![] }
    }

    // non-empty default. covers [embedded data, absolute path, CWD].
    pub fn new_default() -> Self {
        PathAnchor {
            anchors: vec![
                Box::new(EmbeddedAnchor::new()),
                Box::new(FileSystemAnchor::new_cwd()),
            ],
        }
    }

    pub fn new_embedded() -> Self {
        PathAnchor {
            anchors: vec![Box::new(EmbeddedAnchor::new())],
        }
    }

    pub fn new_cwd() -> Self {
        PathAnchor {
            anchors: vec![Box::new(FileSystemAnchor::new_cwd())],
        }
    }

    pub fn new_filesystem<P: Into<PathBuf>>(path: P) -> Self {
        PathAnchor {
            anchors: vec![Box::new(FileSystemAnchor::new(path))],
        }
    }

    // push another PathResolver to the anchor
    pub fn push(&mut self, other: Box<dyn PathResolver>) {
        self.anchors.push(other);
    }

    // append another PathAnchor after this
    pub fn append(&mut self, other: &mut PathAnchor) {
        self.anchors.append(&mut other.anchors);
    }

    // return first existing data source found in the anchor, or error
    pub fn resolve<P: AsRef<Path>>(&self, path: P) -> Result<DataSource, ConfigError> {
        self.first_existing(path.as_ref())
            .ok_or(self.resolution_failure(path))
    }

    // check if a path can be found in the anchor
    pub fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        self.first_existing(path.as_ref()).is_some()
    }

    // return first existing data source found in the anchor
    pub fn first_existing<P: AsRef<Path>>(&self, path: P) -> Option<DataSource> {
        self.all_candidates(path).find(|ds| ds.exists())
    }

    // return first existing path found in the anchor
    pub(crate) fn first_existing_path<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        self.filesystem_roots()
            .iter()
            .map(|root| root.join(path.as_ref()))
            .find(|p| p.exists())
    }

    // iterate all data source candidates in the anchor
    pub fn all_candidates<'a, P: AsRef<Path> + 'a>(
        &'a self,
        path: P,
    ) -> impl Iterator<Item = DataSource> + 'a {
        self.anchors
            .iter()
            .filter_map(move |anchor| anchor.candidate(path.as_ref()))
    }

    // create a error with a list of candidate from the anchor
    pub fn resolution_failure<P: AsRef<Path>>(&self, path: P) -> ConfigError {
        let target_path = path.as_ref().to_string_lossy().into_owned();
        let candidates = self
            .anchors
            .iter()
            .map(move |anchor| anchor.candidate(path.as_ref()))
            .map(|cand| format!("{:?}", cand))
            .collect();
        ConfigError::PathResolution(target_path, candidates)
    }

    // filesystem roots for manual resulution
    pub fn filesystem_roots(&self) -> Vec<&PathBuf> {
        self.anchors
            .iter()
            .filter_map(|anchor| anchor.filesystem_roots())
            .collect()
    }
}

// anchor that searchs specified directory
#[derive(Default, Debug, Clone)]
pub struct FileSystemAnchor {
    root: PathBuf,
}

impl PathResolver for FileSystemAnchor {
    fn candidate(&self, path: &Path) -> Option<DataSource> {
        Some(DataSource::File(self.root.join(path)))
    }

    fn filesystem_roots(&self) -> Option<&PathBuf> {
        Some(&self.root)
    }
}

impl FileSystemAnchor {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        FileSystemAnchor { root: path.into() }
    }

    pub fn new_cwd() -> Self {
        FileSystemAnchor {
            root: PathBuf::new(),
        }
    }
}

// anchor that returns embedded data
#[derive(Default, Debug, Clone)]
pub struct EmbeddedAnchor {}

impl PathResolver for EmbeddedAnchor {
    fn candidate(&self, path: &Path) -> Option<DataSource> {
        path.to_str().and_then(|pathstr| match pathstr {
            DEFAULT_CHAR_DEF_FILE => Some(DataSource::Borrowed(DEFAULT_CHAR_DEF_BYTES)),
            DEFAULT_REWRITE_DEF_FILE => Some(DataSource::Borrowed(DEFAULT_REWRITE_DEF_BYTES)),
            DEFAULT_UNK_DEF_FILE => Some(DataSource::Borrowed(DEFAULT_UNK_DEF_BYTES)),
            DEFAULT_SETTING_FILE => Some(DataSource::Borrowed(DEFAULT_SETTING_BYTES)),
            _ => None,
        })
    }
}

impl EmbeddedAnchor {
    pub fn new() -> Self {
        EmbeddedAnchor {}
    }
}
