// SPDX-License-Identifier: Apache-2.0

use crate::CharsIter;

// YAML 1.2.2:
//
//      In YAML block styles, structure is determined by indentation. In
//      general, indentation is defined as a zero or more space characters at
//      the start of a line.
//      To maintain portability, tab characters must not be used in indentation
//
//  ---------------------------------------------------------------------------
//  YAML 1.2.2:
//
//      In this case, both the “-” indicator and the following spaces are
//      considered to be part of the indentation of the nested collection. Note
//      that it is not possible to specify node properties for such a
//      collection.
//
pub(crate) fn process_indent(iter: &mut CharsIter) -> usize {
    let mut count = 0usize;
    while let Some(c) = iter.peek() {
        if c == ' ' {
            count += 1;
            iter.next();
        } else if c == '-' && iter.as_str().starts_with("- ") {
            return count + 2;
        } else {
            return count;
        }
    }
    count
}
