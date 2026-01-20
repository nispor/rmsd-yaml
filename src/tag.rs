// SPDX-License-Identifier: Apache-2.0

use crate::{YamlParser, YamlValueData};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct YamlTag {
    pub name: String,
    pub data: YamlValueData,
}

impl<'a> YamlParser<'a> {
    // TODO:
    //   * It is possible to override this default behavior by providing an
    //     explicit “TAG” directive associating a different prefix for this
    //     handle. e.g. `%TAG !! tag:example.com,2000:app/`
    pub(crate) fn handle_tag(&mut self) -> Option<String> {
        let tag_name = self.scanner.peek_till_linebreak_or_space();

        if let Some(tag) = tag_name.strip_prefix("!!") {
            let ret = format!("<tag:yaml.org,2002:{tag}>");
            self.scanner.advance_till_linebreak_or_space();
            return Some(ret);
        } else if let Some(tag) = tag_name.strip_prefix("!") {
            let ret = tag.to_string();
            self.scanner.advance_till_linebreak_or_space();
            return Some(ret);
        } else if !tag_name.is_empty() {
            log::trace!("Unknown tag {tag_name}");
        }
        None
    }
}
