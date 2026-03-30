// Test harness setup: keep the event loop alive for cross-thread callbacks.
//
// The napi player's ThreadsafeFunction is unref'd (correctly — it shouldn't
// prevent process exit in production). But Node's test runner exits when the
// event loop has no refs. This ref'd timer keeps it alive during test
// execution. It unref's itself once all tests have reported, allowing the
// process to exit normally.
import { after } from "node:test";

const keepAlive = setInterval(() => {}, 60000);
after(() => clearInterval(keepAlive));
