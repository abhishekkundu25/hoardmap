//! hoardmap: scan a directory tree, serve it to the browser as a directory list, a
//! treemap and an extension list. Nothing here deletes a file.
//!
//! - [`tree`]: the scanned tree, an arena of nodes with children sorted by size.
//! - [`scan`]: builds a [`tree::Tree`] from the file system, in parallel, reporting progress.
//! - [`treemap`]: squarified layout of a subtree into rectangles, and extension colours.
//! - [`icicle`]: one row per depth, children under their parent by bytes.
//! - [`server`]: the HTTP server, browser presence and the exit-when-closed rule.
//! - [`api`]: the JSON endpoints the page calls, over the scan state.
//! - [`cli`]: command-line options.
//! - [`access`]: the Full Disk Access check, its Settings pane, and relaunching the app.

pub mod access;
pub mod api;
pub mod cli;
pub mod icicle;
pub mod scan;
pub mod server;
pub mod tree;
pub mod treemap;
