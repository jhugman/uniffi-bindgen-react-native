/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/// Wrap a docstring in a JSDoc `/** ... */` block at the given indentation.
///
/// Any `*/` in the docstring is escaped, since it would otherwise close the
/// block early. TypeScript does not permit nested
/// block comments.
pub(super) fn format_docstring_at(docstring: &str, indent_spaces: usize) -> String {
    let escaped = docstring.replace("*/", "*\\/");
    let middle = textwrap::indent(&textwrap::dedent(&escaped), " * ");
    let wrapped = format!("/**\n{middle}\n */");
    textwrap::indent(&wrapped, &" ".repeat(indent_spaces))
}

pub(super) fn format_docstring(docstring: &str) -> String {
    format_docstring_at(docstring, 0)
}

pub(super) fn format_docstring_indented(docstring: &str) -> String {
    format_docstring_at(docstring, 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The only `*/` in a formatted docstring should be closing it.
    fn closes_exactly_once(formatted: &str) -> bool {
        formatted.matches("*/").count() == 1 && formatted.ends_with("*/")
    }

    #[test]
    fn escapes_a_lone_marker() {
        assert_eq!(
            format_docstring("See the */ marker."),
            "/**\n * See the *\\/ marker.\n */"
        );
    }

    #[test]
    fn escapes_a_nested_block_comment() {
        assert!(closes_exactly_once(&format_docstring(
            "foo(bar: [/* ... */])"
        )));
    }

    #[test]
    fn leaves_docstrings_without_markers_alone() {
        assert_eq!(format_docstring("Plain text."), "/**\n * Plain text.\n */");
    }

    #[test]
    fn escapes_at_indentation() {
        assert!(closes_exactly_once(&format_docstring_indented(
            "Docs with */ in them."
        )));
    }
}
