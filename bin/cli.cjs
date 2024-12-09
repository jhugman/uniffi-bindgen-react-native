/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const { execSync } = require("child_process");
const path = require("path");
const fs = require("fs");

function getRootDir() {
  const currentDir = __dirname;
  const repoMarker = "uniffi-bindgen-react-native";

  // Check if the current directory is within the `uniffi-bindgen-react-native` repo
  if (currentDir.includes(repoMarker)) {
    let dir = currentDir;
    while (dir !== path.parse(dir).root) {
      if (fs.existsSync(path.join(dir, "Cargo.toml"))) {
        return dir;
      }
      dir = path.dirname(dir);
    }
  }

  // Fallback to using require.resolve
  const resolvedPath = require.resolve("uniffi-bindgen-react-native");
  return resolvedPath.replace(/\/typescript\/src\/index\.ts$/, "");
}

// Get the root directory
const rootDir = getRootDir();

const args = process.argv.slice(2);
if (args.length === 1 && args[0] === "--path") {
  console.log(`${rootDir}/bin/cli`);
  process.exit(0);
}

// Construct the path to the Cargo.toml file
const manifestPath = path.join(rootDir, "crates/ubrn_cli/Cargo.toml");

if (!fs.existsSync(path.join(rootDir, "target"))) {
  console.log(
    "🤖 Building the uniffi-bindgen-react-native command… this is only needed first time",
  );
}

// Run the cargo command
const command = `cargo run --quiet --manifest-path "${manifestPath}" -- ${args.join(" ")}`;
try {
  execSync(command, { stdio: "inherit" });
} catch (e) {
  // cargo run errors are reported to stderr already.
  // We do not want to show the JS error.
  // All we have left is to exit with a non-zero exit code.
  process.exit(1);
}
