// SPDX-License-Identifier: Apache-2.0

/// Prefer unquoted string and use double quoted string if any of below:
///     * Line is longer than `max_width`
///     * Has non-printable character
///     * Has NS_ESC_XXX characters
pub(crate) fn to_scalar_string(
    indent_count: usize,
    input: &str,
    max_width: usize,
) -> String {
    // TODO: Escape non-printable character
    // TODO: Escape NS_ESC_XXX characters
    // TODO: Break long line
    if indent_count + input.chars().count() < max_width {
        input.to_string()
    } else {
        format!("\"{input}\"")
    }
}
