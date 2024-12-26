// SPDX-License-Identifier: Apache-2.0

use crate::CharsIter;

// YAML 1.2.2:
//      In YAML block styles, structure is determined by indentation. In
//      general, indentation is defined as a zero or more space characters at
//      the start of a line.
//      To maintain portability, tab characters must not be used in indentation
pub(crate) fn process_indent(iter: &mut CharsIter) -> usize {
    let mut count = 0usize;
    while let Some(c) = iter.peek() {
        if c == ' ' {
            count += 1;
            iter.next();
        } else {
            return count;
        }
    }
    count
}
