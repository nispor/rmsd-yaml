// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub(crate) enum YamlState {
    InStream,
    InDocument,
    InBlockMapKey(usize),
    InBlockMapValue,
    InBlockSequnce(usize),
    InFlowMapKey(usize),
    InFlowMapValue,
    InFlowSequnce(usize),
    InScalar,
    #[default]
    EndOfFile,
}

impl YamlState {
    pub(crate) fn is_stream(&self) -> bool {
        self == &Self::InDocument
    }

    pub(crate) fn is_document(&self) -> bool {
        self == &Self::InDocument
    }

    pub(crate) fn is_flow(&self) -> bool {
        matches!(
            self,
            &Self::InFlowMapKey(_)
                | &Self::InFlowMapValue
                | &Self::InFlowSequnce(_)
        )
    }

    pub(crate) fn is_block_map_key(&self) -> bool {
        matches!(self, &Self::InBlockMapKey(_))
    }

    pub(crate) fn is_seq(&self) -> bool {
        matches!(self, &Self::InBlockSequnce(_) | &Self::InFlowSequnce(_))
    }

    pub(crate) fn is_scalar(&self) -> bool {
        self == &Self::InScalar
    }

    /// In map or sequence
    pub(crate) fn is_container(&self) -> bool {
        matches!(
            self,
            &Self::InBlockMapKey(_)
                | &Self::InBlockMapValue
                | &Self::InBlockSequnce(_)
                | &Self::InFlowMapKey(_)
                | &Self::InFlowMapValue
                | &Self::InFlowSequnce(_)
        )
    }

    pub(crate) fn eof(&self) -> bool {
        self == &Self::EndOfFile
    }
}
