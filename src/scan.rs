//! Builds a [`crate::tree::Tree`] from the file system: a parallel walk that stays on
//! the root's device, counts hard links once, records allocated size and the latest
//! modification time per subtree, and reports progress while it runs.
