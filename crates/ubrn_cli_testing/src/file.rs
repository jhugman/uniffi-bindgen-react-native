/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::path::Path;

use camino::Utf8PathBuf;

/// A builder for file matchers to be used in tests
#[derive(Debug, Clone)]
pub struct File {
    path: Utf8PathBuf,
    content_matchers: Vec<ContentMatcher>,
}

/// Types of content matchers for files
#[derive(Debug, Clone)]
pub enum ContentMatcher {
    /// Match file content that contains the specified substring
    Contains(String),
    /// Match file content that does not contain the specified substring
    DoesNotContain(String),
}

impl File {
    /// Create a new file matcher for a given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: Utf8PathBuf::from(path.as_ref().to_string_lossy().to_string()),
            content_matchers: Vec::new(),
        }
    }

    /// Add a matcher to check if the file contains a substring
    pub fn contains(mut self, substring: &str) -> Self {
        self.content_matchers
            .push(ContentMatcher::Contains(substring.to_string()));
        self
    }

    /// Add a matcher to check if the file does not contain a substring
    pub fn does_not_contain(mut self, substring: &str) -> Self {
        self.content_matchers
            .push(ContentMatcher::DoesNotContain(substring.to_string()));
        self
    }

    /// Get the file path
    pub fn path(&self) -> &Utf8PathBuf {
        &self.path
    }

    /// Get the content matchers
    pub fn content_matchers(&self) -> &[ContentMatcher] {
        &self.content_matchers
    }
}
