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

use std::path::{Path, PathBuf};

use super::ConfigError;

#[derive(Default, Debug, Clone)]
pub(crate) struct PathResolver {
    roots: Vec<PathBuf>,
}

impl PathResolver {
    pub(crate) fn with_capacity(capacity: usize) -> PathResolver {
        PathResolver {
            roots: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn add<P: Into<PathBuf>>(&mut self, path: P) {
        self.roots.push(path.into())
    }

    pub(crate) fn contains<P: AsRef<Path>>(&self, path: P) -> bool {
        let query = path.as_ref();
        self.roots.iter().any(|p| p.as_path() == query)
    }

    pub fn first_existing<P: AsRef<Path> + Clone>(&self, path: P) -> Option<PathBuf> {
        self.all_candidates(path).find(|p| p.exists())
    }

    pub fn resolution_failure<P: AsRef<Path> + Clone>(&self, path: P) -> ConfigError {
        let candidates = self
            .all_candidates(path.clone())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        ConfigError::PathResolution(path.as_ref().to_string_lossy().into_owned(), candidates)
    }

    pub fn all_candidates<'a, P: AsRef<Path> + Clone + 'a>(
        &'a self,
        path: P,
    ) -> impl Iterator<Item = PathBuf> + 'a {
        self.roots.iter().map(move |root| root.join(path.clone()))
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}
