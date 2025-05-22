# File Matching API

## Overview

The File Matching API provides a convenient way to test file operations in your code. It allows you to:

- Record file writes during test execution
- Assert on file content using flexible matchers
- Match files by path suffix to handle variable paths
- Provide detailed error messages for failed assertions

## Usage

### Basic Example

```rust
use ubrn_cli_testing::{start_recording, stop_recording, File, assert_files};
use ubrn_common::write_file;

#[test]
fn test_my_function_writes_correct_files() {
    // Start recording file operations
    start_recording();

    // Call your function that writes files
    my_function_that_writes_files();

    // Assert on the files that were written
    assert_files(&[
        // Match a file with exact path suffix
        File::new("output.json")
            .contains("\"status\": \"success\"")
            .does_not_contain("error"),

        // Match another file using path suffix
        File::new("/src/generated/types.kt")
            .contains("class User")
    ]);

    // Clean up recording state
    stop_recording();
}
```

### Available Matchers

The API provides two content matchers:

1. `.contains(substring)` - Asserts that the file content contains the specified substring
2. `.does_not_contain(substring)` - Asserts that the file content does not contain the substring

### Path Matching

File paths are matched using suffix matching, which means you only need to provide the unique part of the path:

```rust
// This will match any file ending with "/src/main.kt"
File::new("/src/main.kt")

// This will match any file named "config.json"
File::new("config.json")
```

### Functions

- `start_recording()` - Begins recording file operations
- `stop_recording()` - Stops recording and clears recorded files
- `assert_files(files)` - Asserts that files matching the provided patterns were written
- `files_match(files)` - Returns true if files match the patterns, false otherwise

## Integration with Command Recording

The file matching API integrates seamlessly with the existing command recording functionality:

```rust
use ubrn_cli_testing::{start_recording, stop_recording, Command, assert_commands, File, assert_files};

#[test]
fn test_function_generates_files_and_runs_commands() {
    start_recording();

    my_function();

    // Assert on commands
    assert_commands(&[
        Command::new("npm").arg("install"),
        Command::new("cargo").arg("build")
    ]);

    // Assert on files
    assert_files(&[
        File::new("package.json").contains("\"name\": \"my-package\""),
        File::new("src/index.js").contains("export default")
    ]);

    stop_recording();
}
```
