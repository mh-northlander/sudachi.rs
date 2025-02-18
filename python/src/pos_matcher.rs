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

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyIterator, PyTuple};

use sudachi::analysis::stateless_tokenizer::DictionaryAccess;
use sudachi::pos::PosMatcher;

use crate::dictionary::PyDicData;
use crate::errors;
use crate::morpheme::PyMorpheme;

/// A part-of-speech matcher which checks if a morpheme belongs to a set of part of speech.
///
/// Create using Dictionary.pos_matcher method.
///
/// Use `__call__(m: Morpheme) -> bool` to check whether a morpheme has matching POS.
#[pyclass(module = "sudachipy.pos_matcher", name = "PosMatcher")]
pub struct PyPosMatcher {
    matcher: PosMatcher,
    dic: Arc<PyDicData>,
}

impl PyPosMatcher {
    pub(crate) fn create<'py>(
        dic: &'py Arc<PyDicData>,
        data: &Bound<'py, PyAny>,
    ) -> PyResult<PyPosMatcher> {
        if data.is_callable() {
            Self::create_from_fn(dic, data)
        } else {
            let iter = data.try_iter()?;
            Self::create_from_iter(dic, &iter)
        }
    }

    fn create_from_fn(dic: &Arc<PyDicData>, func: &Bound<PyAny>) -> PyResult<Self> {
        let mut data = Vec::new();
        for (pos_id, pos) in dic.pos.iter().enumerate() {
            if func.call1((pos,))?.downcast::<PyBool>()?.is_true() {
                data.push(pos_id as u16);
            }
        }
        Ok(Self {
            matcher: PosMatcher::new(data),
            dic: dic.clone(),
        })
    }

    fn create_from_iter(dic: &Arc<PyDicData>, data: &Bound<PyIterator>) -> PyResult<Self> {
        let mut result = Vec::new();
        for item in data {
            let item = item?;
            let item = item.downcast::<PyTuple>()?;
            Self::match_pos_elements(&mut result, dic.as_ref(), item)?;
        }
        Ok(Self {
            matcher: PosMatcher::new(result),
            dic: dic.clone(),
        })
    }

    fn match_pos_elements(
        data: &mut Vec<u16>,
        dic: &PyDicData,
        elem: &Bound<PyTuple>,
    ) -> PyResult<()> {
        let start_len = data.len();

        let elen = elem.len();
        for (pos_id, pos) in dic.grammar().pos_list.iter().enumerate() {
            let check = |idx: usize| -> PyResult<bool> {
                let x = elem.get_item(idx)?;
                if x.is_none() {
                    return Ok(false);
                }
                Ok(x.str()?.to_str()? != pos[idx])
            };
            if elen > 0 && check(0)? {
                continue;
            }
            if elen > 1 && check(1)? {
                continue;
            }
            if elen > 2 && check(2)? {
                continue;
            }
            if elen > 3 && check(3)? {
                continue;
            }
            if elen > 4 && check(4)? {
                continue;
            }
            if elen > 5 && check(5)? {
                continue;
            }
            data.push(pos_id as u16);
        }

        if start_len == data.len() {
            errors::wrap(Err(format!(
                "POS {:?} did not match any elements",
                elem.repr()?
            )))
        } else {
            Ok(())
        }
    }
}

#[pymethods]
impl PyPosMatcher {
    /// Checks whether a morpheme has matching POS.
    ///
    /// :param m: a morpheme to check.
    /// :return: if morpheme has matching POS.
    ///
    /// :type m: Morpheme
    pub fn __call__<'py>(&'py self, py: Python<'py>, m: &'py PyMorpheme) -> bool {
        let pos_id = m.part_of_speech_id(py);
        self.matcher.matches_id(pos_id)
    }

    pub fn __str__(&self) -> String {
        format!("<PosMatcher:{} pos>", self.matcher.num_entries())
    }

    pub fn __iter__(&self) -> PyPosIter {
        PyPosIter::new(self.matcher.entries(), self.dic.clone())
    }

    pub fn __len__(&self) -> usize {
        self.matcher.num_entries()
    }

    /// Returns a POS matcher which matches a POS if any of two matchers would match it.
    pub fn __or__(&self, other: &Self) -> Self {
        assert_eq!(
            Arc::as_ptr(&self.dic),
            Arc::as_ptr(&other.dic),
            "incompatible dictionaries"
        );
        let matcher = self.matcher.union(&other.matcher);
        Self {
            dic: self.dic.clone(),
            matcher,
        }
    }

    /// Returns a POS matcher which matches a POS if both matchers would match it at the same time.
    pub fn __and__(&self, other: &Self) -> Self {
        assert_eq!(
            Arc::as_ptr(&self.dic),
            Arc::as_ptr(&other.dic),
            "incompatible dictionaries"
        );
        let matcher = self.matcher.intersection(&other.matcher);
        Self {
            dic: self.dic.clone(),
            matcher,
        }
    }

    /// Returns a POS matcher which matches a POS if self would match the POS and other would not match the POS.
    pub fn __sub__(&self, other: &Self) -> Self {
        assert_eq!(
            Arc::as_ptr(&self.dic),
            Arc::as_ptr(&other.dic),
            "incompatible dictionaries"
        );
        let matcher = self.matcher.difference(&other.matcher);
        Self {
            dic: self.dic.clone(),
            matcher,
        }
    }

    /// Returns a POS matcher which matches all POS tags except ones defined in the current POS matcher.
    pub fn __invert__(&self) -> Self {
        let max_id = self.dic.pos.len();
        // map -> filter chain is needed to handle exactly u16::MAX POS entries
        let values = (0..max_id)
            .map(|x| x as u16)
            .filter(|id| !self.matcher.matches_id(*id));
        let matcher = PosMatcher::new(values);
        Self {
            matcher,
            dic: self.dic.clone(),
        }
    }
}

/// An iterator over POS tuples in the PosPatcher
#[pyclass(module = "sudachipy.pos_matcher", name = "PosMatcherIterator")]
pub struct PyPosIter {
    data: Vec<u16>,
    dic: Arc<PyDicData>,
    index: usize,
}

impl PyPosIter {
    fn new(data: impl Iterator<Item = u16>, dic: Arc<PyDicData>) -> Self {
        let mut result: Vec<u16> = data.collect();
        result.sort();
        Self {
            data: result,
            dic,
            index: 0,
        }
    }
}

#[pymethods]
impl PyPosIter {
    fn __iter__(slf: Bound<Self>) -> Bound<Self> {
        slf
    }

    fn __next__<'py>(&'py mut self, py: Python<'py>) -> Option<&Bound<'py, PyTuple>> {
        let idx = self.index;
        self.index += 1;
        if idx >= self.data.len() {
            return None;
        }
        let pos_id = self.data[idx];
        let pos = &self.dic.pos[pos_id as usize];
        Some(pos.bind(py))
    }
}
