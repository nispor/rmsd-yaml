// SPDX-License-Identifier: Apache-2.0

use crate::{ErrorKind, YamlError, YamlEvent, YamlTreeParser};

impl<'a> YamlTreeParser<'a> {
    pub(crate) fn handle_in_block_seq(&mut self) -> Result<(), YamlError> {
        todo!()
    }
}
