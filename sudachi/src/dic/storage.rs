/*
 *  Copyright (c) 2021-2024 Works Applications Co., Ltd.
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 *   Unless required by applicable law or agreed to in writing, software
 *  distributed under the License is distributed on an "AS IS" BASIS,
 *  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *  See the License for the specific language governing permissions and
 *  limitations under the License.
 */

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;
use nom::AsBytes;

use crate::config::DataSource;
use crate::error::{SudachiError, SudachiResult};

pub enum Storage {
    File(Mmap),
    Borrowed(&'static [u8]),
    Owned(Vec<u8>),
}

impl Storage {
    fn from_path<P: AsRef<Path>>(path: P) -> SudachiResult<Self> {
        let file = File::open(path)?;
        let mapping = unsafe { Mmap::map(&file) }?;
        Ok(Storage::File(mapping))
    }
}

impl TryFrom<DataSource> for Storage {
    type Error = SudachiError;

    fn try_from(value: DataSource) -> SudachiResult<Self> {
        match value {
            DataSource::File(p) => {
                Self::from_path(&p).map_err(|e| e.with_context(p.as_os_str().to_string_lossy()))
            }
            DataSource::Borrowed(b) => Ok(Self::Borrowed(b)),
            DataSource::Owned(v) => Ok(Self::Owned(v)),
        }
    }
}

impl AsRef<[u8]> for Storage {
    fn as_ref(&self) -> &[u8] {
        match self {
            Storage::File(m) => m.as_bytes(),
            Storage::Borrowed(b) => b,
            Storage::Owned(v) => v,
        }
    }
}

pub struct SudachiDicData {
    // system dictionary
    system: Storage,
    // user dictionaries
    user: Vec<Storage>,
}

impl SudachiDicData {
    pub fn new(system: Storage) -> Self {
        Self {
            system,
            user: Vec::new(),
        }
    }

    pub fn add_user(&mut self, user: Storage) {
        self.user.push(user)
    }

    pub fn system(&self) -> &[u8] {
        self.system.as_ref()
    }

    /// # Safety
    /// Call this function only after system dictionary data is ready.
    pub unsafe fn system_static_slice(&self) -> &'static [u8] {
        std::mem::transmute(self.system())
    }

    pub(crate) fn user_static_slice(&self) -> Vec<&'static [u8]> {
        let mut result = Vec::with_capacity(self.user.len());
        for u in self.user.iter() {
            let slice: &'static [u8] = unsafe { std::mem::transmute(u.as_ref()) };
            result.push(slice);
        }
        result
    }
}
