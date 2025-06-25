/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// A builder for command matchers to be used in tests
#[derive(Debug, Clone)]
pub struct Command {
    program: String,
    args: Vec<ArgMatcher>,
    cwd: Option<PathBuf>,
    env: HashMap<String, String>,
}

impl Command {
    /// Create a new command matcher for a given program name
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
        }
    }

    /// Add an argument to match exactly
    pub fn arg(mut self, arg: &str) -> Self {
        self.args.push(ArgMatcher::Exact(arg.to_string()));
        self
    }

    /// Set the working directory to match exactly
    pub fn cwd<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.cwd = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the working directory to match by suffix
    pub fn cwd_suffix(mut self, suffix: &str) -> Self {
        self.cwd = Some(PathBuf::from(suffix));
        self
    }

    /// Add an argument to match by suffix
    pub fn arg_suffix(mut self, suffix: &str) -> Self {
        self.args.push(ArgMatcher::Suffix(suffix.to_string()));
        self
    }

    /// Add a matching argument pair (two consecutive arguments)
    pub fn arg_pair(mut self, key: &str, value: &str) -> Self {
        self.args
            .push(ArgMatcher::Pair(key.to_string(), value.to_string()));
        self
    }

    /// Add a matching argument pair where the value is matched by suffix
    /// This is useful for arguments like file paths where only the end of the path matters
    pub fn arg_pair_suffix(mut self, key: &str, value_suffix: &str) -> Self {
        self.args.push(ArgMatcher::PairSuffix(
            key.to_string(),
            value_suffix.to_string(),
        ));
        self
    }

    /// Add an environment variable to match exactly
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }

    /// Get the program name
    pub(crate) fn program(&self) -> &str {
        &self.program
    }

    /// Get the working directory matcher
    pub(crate) fn get_cwd(&self) -> Option<&PathBuf> {
        self.cwd.as_ref()
    }

    /// Get the list of argument matchers
    pub(crate) fn args(&self) -> &[ArgMatcher] {
        &self.args
    }

    /// Get the environment variables
    pub(crate) fn get_env(&self) -> &HashMap<String, String> {
        &self.env
    }
}

/// Types of argument matchers for command assertions
#[derive(Debug, Clone)]
pub enum ArgMatcher {
    /// Match an exact argument string
    Exact(String),
    /// Match an argument that ends with a suffix
    Suffix(String),
    /// Match a key-value pair of arguments (two consecutive arguments)
    Pair(String, String),
    /// Match a key-value pair where the key is exact but value is matched by suffix
    PairSuffix(String, String),
}
