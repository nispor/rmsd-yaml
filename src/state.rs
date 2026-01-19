// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub(crate) enum YamlState {
    InBlockMapKey,
    InBlockMapValue,
    InBlockSequnce,
    InFlowMapKey,
    InFlowMapValue,
    InFlowSequnce,
    #[default]
    EndOfFile,
}

impl YamlState {
    pub(crate) fn is_flow(&self) -> bool {
        matches!(
            self,
            &Self::InFlowMapKey | &Self::InFlowMapValue | &Self::InFlowSequnce
        )
    }

    pub(crate) fn is_block_map_key(&self) -> bool {
        self == &Self::InBlockMapKey
    }

    pub(crate) fn is_block_map_value(&self) -> bool {
        self == &Self::InBlockMapValue
    }

    pub(crate) fn is_seq(&self) -> bool {
        matches!(self, &Self::InBlockSequnce | &Self::InFlowSequnce)
    }

    pub(crate) fn is_block_seq(&self) -> bool {
        self == &Self::InBlockSequnce
    }

    /// In map or sequence
    pub(crate) fn is_container(&self) -> bool {
        matches!(
            self,
            &Self::InBlockMapKey
                | &Self::InBlockMapValue
                | &Self::InBlockSequnce
                | &Self::InFlowMapKey
                | &Self::InFlowMapValue
                | &Self::InFlowSequnce
        )
    }

    pub(crate) fn eof(&self) -> bool {
        self == &Self::EndOfFile
    }
}
