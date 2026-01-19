// SPDX-License-Identifier: Apache-2.0

use crate::YamlTreeParser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YamlTag {
    YamlOrg2002Str,
    YamlOrg2002Map,
    YamlOrg2002Sequence,
    YamlOrg2002Bool,
    YamlOrg2002Int,
    YamlOrg2002Float,
}

impl std::fmt::Display for YamlTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YamlOrg2002Str => write!(f, "<tag:yaml.org,2002:str>"),
            Self::YamlOrg2002Map => write!(f, "<tag:yaml.org,2002:map>"),
            Self::YamlOrg2002Sequence => write!(f, "<tag:yaml.org,2002:seq>"),
            Self::YamlOrg2002Bool => write!(f, "<tag:yaml.org,2002:bool>"),
            Self::YamlOrg2002Int => write!(f, "<tag:yaml.org,2002:int>"),
            Self::YamlOrg2002Float => write!(f, "<tag:yaml.org,2002:float>"),
        }
    }
}

impl<'a> YamlTreeParser<'a> {
    // TODO:
    //   * It is possible to override this default behavior by providing an
    //     explicit “TAG” directive associating a different prefix for this
    //     handle. e.g. `%TAG !! tag:example.com,2000:app/`
    pub(crate) fn handle_tag(&mut self) -> Option<YamlTag> {
        let ret = match self.scanner.peek_till_linebreak_or_space() {
            "!!str" | "<tag:yaml.org,2002:str>" => {
                Some(YamlTag::YamlOrg2002Str)
            }
            "!!int" | "<tag:yaml.org,2002:int>" => {
                Some(YamlTag::YamlOrg2002Int)
            }
            "!!bool" | "<tag:yaml.org,2002:bool>" => {
                Some(YamlTag::YamlOrg2002Bool)
            }
            "!!float" | "<tag:yaml.org,2002:float>" => {
                Some(YamlTag::YamlOrg2002Float)
            }
            "!!seq" | "<tag:yaml.org,2002:seq>" => {
                Some(YamlTag::YamlOrg2002Sequence)
            }
            "!!map" | "<tag:yaml.org,2002:map>" => {
                Some(YamlTag::YamlOrg2002Map)
            }
            unknown_tag => {
                log::trace!("Unknown tag {unknown_tag}");
                None
            }
        };
        if ret.is_some() {
            self.scanner.advance_till_linebreak_or_space();
        }
        ret
    }
}
