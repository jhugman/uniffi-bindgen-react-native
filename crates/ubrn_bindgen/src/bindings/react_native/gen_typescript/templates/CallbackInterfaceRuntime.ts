// Magic number for the Rust proxy to call using the same mechanism as every other method,
// to free the callback once it's dropped by Rust.
const IDX_CALLBACK_FREE = 0;
// Callback return codes
const UNIFFI_CALLBACK_SUCCESS = 0;
const UNIFFI_CALLBACK_ERROR = 1;
const UNIFFI_CALLBACK_UNEXPECTED_ERROR = 2;
