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

use std::convert::TryFrom;

use serde::Deserialize;

use super::ConfigError;
use crate::dic::subset::InfoSubset;
use crate::error::SudachiError;

#[derive(Deserialize, Clone, Copy, Debug, Eq, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceProjection {
    #[default]
    Surface,
    Normalized,
    Reading,
    Dictionary,
    DictionaryAndSurface,
    NormalizedAndSurface,
    NormalizedNouns,
}

impl SurfaceProjection {
    /// Return required InfoSubset for the current projection type
    pub fn required_subset(&self) -> InfoSubset {
        match *self {
            SurfaceProjection::Surface => InfoSubset::empty(),
            SurfaceProjection::Normalized => InfoSubset::NORMALIZED_FORM,
            SurfaceProjection::Reading => InfoSubset::READING_FORM,
            SurfaceProjection::Dictionary => InfoSubset::DIC_FORM_WORD_ID,
            SurfaceProjection::DictionaryAndSurface => InfoSubset::DIC_FORM_WORD_ID,
            SurfaceProjection::NormalizedAndSurface => InfoSubset::NORMALIZED_FORM,
            SurfaceProjection::NormalizedNouns => InfoSubset::NORMALIZED_FORM,
        }
    }
}

impl TryFrom<&str> for SurfaceProjection {
    type Error = SudachiError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "surface" => Ok(SurfaceProjection::Surface),
            "normalized" => Ok(SurfaceProjection::Normalized),
            "reading" => Ok(SurfaceProjection::Reading),
            "dictionary" => Ok(SurfaceProjection::Dictionary),
            "dictionary_and_surface" => Ok(SurfaceProjection::DictionaryAndSurface),
            "normalized_and_surface" => Ok(SurfaceProjection::NormalizedAndSurface),
            "normalized_nouns" => Ok(SurfaceProjection::NormalizedNouns),
            _ => Err(ConfigError::InvalidFormat(format!("unknown projection: {value}")).into()),
        }
    }
}
