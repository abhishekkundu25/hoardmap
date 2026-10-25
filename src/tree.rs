//! The scanned tree. Nodes live in one arena, `Vec<Node>`, the root at index 0, and
//! every directory's children are sorted by logical size, largest first. Sizes and
//! counts on a directory are subtree totals.

use std::path::PathBuf;

use serde::Serialize;

pub type NodeId = u32;

pub const ROOT: NodeId = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Dir,
    File,
    Symlink,
    /// Sockets, pipes, devices.
    Other,
}

#[derive(Debug, Serialize)]
pub struct Node {
    /// The file name. The root's name is the full path that was scanned.
    pub name: String,
    pub parent: Option<NodeId>,
    pub kind: Kind,
    /// Logical bytes of the subtree. A file reached through a second hard link counts 0.
    pub size: u64,
    /// Bytes allocated on disk for the subtree (blocks × 512).
    pub physical: u64,
    /// Files, symlinks and other non-directories in the subtree; 0 for a non-directory.
    pub files: u32,
    /// Directories in the subtree, this one excluded; 0 for a non-directory.
    pub dirs: u32,
    /// Latest modification time in the subtree, seconds since the Unix epoch.
    pub mtime: i64,
    /// Largest first; empty for a non-directory.
    pub children: Vec<NodeId>,
    /// A later occurrence of a file with more than one hard link: its bytes are counted
    /// under the first occurrence and `size` is 0 here.
    pub hard_link: bool,
}

/// A path the scan could not read, with the error text.
#[derive(Clone, Debug, Serialize)]
pub struct ScanError {
    pub path: String,
    pub error: String,
}

#[derive(Debug)]
pub struct Tree {
    pub nodes: Vec<Node>,
    /// The first [`MAX_ERRORS`] unreadable paths.
    pub errors: Vec<ScanError>,
    /// All unreadable paths, including those not kept in `errors`.
    pub error_count: u64,
}

pub const MAX_ERRORS: usize = 1000;

impl Tree {
    pub fn root(&self) -> &Node {
        &self.nodes[ROOT as usize]
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize)
    }

    /// The full path of a node: the root's name joined with every name below it.
    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut names = Vec::new();
        let mut cur = Some(id);
        while let Some(i) = cur {
            let node = &self.nodes[i as usize];
            names.push(node.name.as_str());
            cur = node.parent;
        }
        names.iter().rev().collect()
    }

    /// `id`, its parent, and so on up to the root.
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut cur = Some(id);
        while let Some(i) = cur {
            out.push(i);
            cur = self.nodes[i as usize].parent;
        }
        out
    }
}

/// The lower-cased extension of a file name, without the dot; empty when there is none
/// or the name starts with its only dot.
pub fn extension(name: &str) -> String {
    match name.rfind('.') {
        Some(0) | None => String::new(),
        Some(i) => name[i + 1..].to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_lowercase_and_dotfiles_have_none() {
        assert_eq!(extension("Photo.JPG"), "jpg");
        assert_eq!(extension(".bashrc"), "");
        assert_eq!(extension("Makefile"), "");
        assert_eq!(extension("archive.tar.gz"), "gz");
        assert_eq!(extension("ends."), "");
    }

    #[test]
    fn path_joins_root_and_names() {
        let tree = Tree {
            nodes: vec![
                Node {
                    name: "/tmp/x".into(),
                    parent: None,
                    kind: Kind::Dir,
                    size: 0,
                    physical: 0,
                    files: 0,
                    dirs: 0,
                    mtime: 0,
                    children: vec![1],
                    hard_link: false,
                },
                Node {
                    name: "a".into(),
                    parent: Some(0),
                    kind: Kind::Dir,
                    size: 0,
                    physical: 0,
                    files: 0,
                    dirs: 0,
                    mtime: 0,
                    children: vec![2],
                    hard_link: false,
                },
                Node {
                    name: "b.txt".into(),
                    parent: Some(1),
                    kind: Kind::File,
                    size: 0,
                    physical: 0,
                    files: 0,
                    dirs: 0,
                    mtime: 0,
                    children: vec![],
                    hard_link: false,
                },
            ],
            errors: vec![],
            error_count: 0,
        };
        assert_eq!(tree.path(2), PathBuf::from("/tmp/x/a/b.txt"));
        assert_eq!(tree.ancestors(2), vec![2, 1, 0]);
    }
}
