// SPDX-License-Identifier: Apache-2.0

use crate::{ErrorKind, YamlError, YamlTree};

pub(crate) struct YamlGraph {}

impl YamlGraph {
    pub(crate) fn compose(trees: Vec<YamlTree>) -> Result<Self, YamlError> {
        todo!()
    }
}
