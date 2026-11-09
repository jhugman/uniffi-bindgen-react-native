# Sync callbacks

Every callback interface with a synchronous method lives here. Async delivery
(`--async`) refuses such interfaces, so this fixture only runs on the sync flavours.

It is intentionally contrived: it panics, asserts params are certain values, and has
no-op methods, to cover all the callback options. For a gentler introduction see the
"examples" directory.
