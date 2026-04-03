/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/// Wrap a docstring in a JSDoc `/** ... */` block at the given indentation.
pub(super) fn format_docstring_at(docstring: &str, indent_spaces: usize) -> String {
    let middle = textwrap::indent(&textwrap::dedent(docstring), " * ");
    let wrapped = format!("/**\n{middle}\n */");
    textwrap::indent(&wrapped, &" ".repeat(indent_spaces))
}

pub(super) fn format_docstring(docstring: &str) -> String {
    format_docstring_at(docstring, 0)
}

pub(super) fn format_docstring_indented(docstring: &str) -> String {
    format_docstring_at(docstring, 4)
}
