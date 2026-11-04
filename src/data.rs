//! Compact immutable arena. Node IDs are stable across both map layouts.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Projects,
    Media,
    Apps,
    Documents,
    System,
    Other,
}
impl Category {
    pub const ALL: [Self; 6] = [
        Self::Projects,
        Self::Media,
        Self::Apps,
        Self::Documents,
        Self::System,
        Self::Other,
    ];
    pub fn label(self, cs: bool) -> &'static str {
        match (self, cs) {
            (Self::Projects, true) => "Projekty",
            (Self::Projects, false) => "Projects",
            (Self::Media, true) => "Média",
            (Self::Media, false) => "Media",
            (Self::Apps, true) => "Aplikace",
            (Self::Apps, false) => "Applications",
            (Self::Documents, true) => "Dokumenty",
            (Self::Documents, false) => "Documents",
            (Self::System, true) => "Systém",
            (Self::System, false) => "System",
            (Self::Other, true) => "Ostatní",
            (Self::Other, false) => "Other",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Directory,
    Symlink,
    Special,
    Unknown,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloudState {
    #[default]
    Local,
    Cached,
    Dataless,
    LegacyPlaceholder,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    Complete,
    Partial,
    Unknown,
}
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub native_path: Option<PathBuf>,
    pub kind: NodeKind,
    pub is_package: bool,
    pub cloud: CloudState,
    pub coverage: Coverage,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub logical: u64,
    pub allocated: u64,
    pub category: Category,
    /// Metadata sizes before hard-link accounting (logical, allocated).
    pub reported_size: Option<(u64, u64)>,
    pub file_identity: Option<(libc::dev_t, libc::ino_t)>,
    /// Another discovered path owns this file's contribution in this revision.
    pub hard_link_owner: Option<usize>,
}
impl Coverage {
    pub fn format(self, size: u64) -> String {
        match self {
            Self::Complete => bytes(size),
            Self::Partial => format!("{} *", bytes(size)),
            Self::Unknown => "?".into(),
        }
    }
}
impl Node {
    pub fn size(&self, allocated: bool) -> u64 {
        if allocated {
            self.allocated
        } else {
            self.logical
        }
    }
}
type ChildOrders = RwLock<HashMap<(usize, bool), Arc<Vec<usize>>>>;
pub struct Dataset {
    pub nodes: Vec<Node>,
    pub files: usize,
    pub run_id: Option<u64>,
    orders: ChildOrders,
}
impl Dataset {
    pub fn demo(files: usize) -> Self {
        let mut data = Self {
            nodes: Vec::with_capacity(files + 128),
            files,
            run_id: None,
            orders: RwLock::new(HashMap::new()),
        };
        data.push("Macintosh HD · DEMO".into(), None, Category::Other, 0);
        let groups = [
            ("Projects", Category::Projects, 32_u64),
            ("Movies", Category::Media, 24),
            ("Applications", Category::Apps, 17),
            ("Documents", Category::Documents, 10),
            ("Library", Category::System, 8),
            ("Downloads", Category::Other, 5),
        ];
        let mut parents = Vec::new();
        for (name, cat, weight) in groups {
            let group = data.push(name.into(), Some(0), cat, 0);
            for j in 0..12 {
                let folder = match cat {
                    Category::Projects => [
                        "orchestra_os",
                        "ByteScape",
                        "The Glass Sea",
                        "Fluid Lab",
                        "Motion Studio",
                        "Archive",
                    ][j % 6],
                    Category::Media => {
                        ["Cinema renders", "Footage", "Sound library", "Textures"][j % 4]
                    }
                    Category::Apps => {
                        ["Creative tools.app", "Development.app", "Utilities.app"][j % 3]
                    }
                    Category::Documents => ["Návrhy", "Research", "Poznámky", "Reference"][j % 4],
                    Category::System => {
                        ["Caches", "Application Support", "Containers", "Developer"][j % 4]
                    }
                    Category::Other => ["Packages", "Images", "Downloads", "Archive"][j % 4],
                };
                let name = if j < 3 {
                    folder.to_string()
                } else {
                    format!("{folder} {:02}", j + 1)
                };
                let id = data.push(name, Some(group), cat, 0);
                parents.push((id, cat, weight));
            }
        }
        let extensions = ["rs", "mov", "bin", "pdf", "cache", "zip"];
        for i in 0..files {
            let (parent, cat, weight) = parents[i % parents.len()];
            let hash = (i as u64).wrapping_mul(2_654_435_761) ^ 0x9e37_79b9;
            let size = (1 + hash % 8192) * weight * 1024;
            let ext = extensions[(i % parents.len()) / 12];
            let label = if i % 29 == 0 {
                "Návrh scény"
            } else {
                "asset"
            };
            data.push(format!("{label}_{i:06}.{ext}"), Some(parent), cat, size);
        }
        for id in (1..data.nodes.len()).rev() {
            let parent = data.nodes[id].parent.unwrap();
            data.nodes[parent].logical += data.nodes[id].logical;
            data.nodes[parent].allocated += data.nodes[id].allocated;
        }
        for n in &mut data.nodes {
            if !n.children.is_empty() {
                n.kind = NodeKind::Directory;
            }
        }
        data
    }
    fn push(
        &mut self,
        name: String,
        parent: Option<usize>,
        category: Category,
        logical: u64,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Node {
            name,
            reported_size: None,
            file_identity: None,
            hard_link_owner: None,
            native_path: None,
            kind: NodeKind::File,
            is_package: false,
            cloud: CloudState::Local,
            coverage: Coverage::Complete,
            parent,
            children: Vec::new(),
            logical,
            allocated: logical.div_ceil(4096) * 4096,
            category,
        });
        if let Some(parent) = parent {
            self.nodes[parent].children.push(id);
        }
        id
    }
    pub fn scanned(nodes: Vec<Node>, run_id: u64) -> Self {
        let files = nodes.iter().filter(|n| n.kind == NodeKind::File).count();
        let data = Self {
            nodes,
            files,
            run_id: Some(run_id),
            orders: RwLock::new(HashMap::new()),
        };
        data.prepare_orders();
        data
    }
    pub fn display_path(&self, id: usize) -> String {
        self.nodes[id].native_path.as_ref().map_or_else(
            || {
                self.path(id)
                    .iter()
                    .map(|&i| self.nodes[i].name.as_str())
                    .collect::<Vec<_>>()
                    .join(" / ")
            },
            |p| p.to_string_lossy().into_owned(),
        )
    }
    pub fn copy_path(&self, id: usize) -> Option<String> {
        self.nodes[id].native_path.as_ref().map_or_else(
            || Some(self.display_path(id)),
            |p| p.to_str().map(str::to_owned),
        )
    }
    pub fn children(&self, root: usize, allocated: bool) -> Arc<Vec<usize>> {
        if let Some(order) = self.orders.read().unwrap().get(&(root, allocated)) {
            return Arc::clone(order);
        }
        let mut children = self.nodes[root].children.clone();
        children.sort_unstable_by(|a, b| {
            self.nodes[*b]
                .size(allocated)
                .cmp(&self.nodes[*a].size(allocated))
                .then(a.cmp(b))
        });
        let children = Arc::new(children);
        self.orders
            .write()
            .unwrap()
            .insert((root, allocated), Arc::clone(&children));
        children
    }
    pub fn prepare_orders(&self) {
        for (id, node) in self.nodes.iter().enumerate() {
            if !node.children.is_empty() {
                self.children(id, true);
                self.children(id, false);
            }
        }
    }
    pub fn path(&self, mut id: usize) -> Vec<usize> {
        let mut result = vec![id];
        while let Some(parent) = self.nodes[id].parent {
            result.push(parent);
            id = parent;
        }
        result.reverse();
        result
    }
}
pub fn bytes(size: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} B")
    } else {
        format!("{value:.1} {}", units[unit])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sibling_names_are_unique_and_order_cache_is_stable() {
        let d = Dataset::demo(10_000);
        for (id, n) in d
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.children.is_empty())
        {
            let names: std::collections::HashSet<_> =
                n.children.iter().map(|&c| &d.nodes[c].name).collect();
            assert_eq!(names.len(), n.children.len());
            for allocated in [false, true] {
                let first = d.children(id, allocated);
                let second = d.children(id, allocated);
                assert!(Arc::ptr_eq(&first, &second));
                assert!(first.windows(2).all(
                    |pair| d.nodes[pair[0]].size(allocated) >= d.nodes[pair[1]].size(allocated)
                ));
            }
        }
    }
    #[test]
    fn hierarchy_is_conservative() {
        let d = Dataset::demo(10_000);
        for (id, n) in d.nodes.iter().enumerate() {
            if !n.children.is_empty() {
                assert_eq!(
                    n.logical,
                    n.children.iter().map(|&c| d.nodes[c].logical).sum::<u64>()
                );
                assert_eq!(
                    n.allocated,
                    n.children
                        .iter()
                        .map(|&c| d.nodes[c].allocated)
                        .sum::<u64>()
                );
                for &c in &n.children {
                    assert_eq!(d.nodes[c].parent, Some(id));
                }
            }
        }
        assert_eq!(
            d.nodes.iter().filter(|n| n.children.is_empty()).count(),
            d.files
        );
        assert_eq!(d.path(d.nodes.len() - 1)[0], 0);
    }
    #[test]
    fn native_paths_remain_distinct_when_display_names_collide() {
        use std::os::unix::ffi::OsStrExt;
        let first = PathBuf::from(std::ffi::OsStr::from_bytes(b"/fixture/raw-\xff"));
        let second = PathBuf::from(std::ffi::OsStr::from_bytes(b"/fixture/raw-\xfe"));
        let nodes = [first.clone(), second.clone()]
            .into_iter()
            .map(|path| Node {
                name: path.file_name().unwrap().to_string_lossy().into_owned(),
                reported_size: None,
                file_identity: None,
                hard_link_owner: None,
                native_path: Some(path),
                kind: NodeKind::File,
                is_package: false,
                cloud: CloudState::Local,
                coverage: Coverage::Complete,
                parent: None,
                children: vec![],
                logical: 1,
                allocated: 512,
                category: Category::Other,
            })
            .collect();
        // Model-level proof only: the local APFS volume rejects such filenames.
        let d = Dataset::scanned(nodes, 9);
        assert_eq!(d.nodes[0].name, d.nodes[1].name);
        assert_ne!(d.nodes[0].native_path, d.nodes[1].native_path);
        assert_eq!(d.nodes[0].native_path.as_ref().unwrap(), &first);
        assert_eq!(d.nodes[1].native_path.as_ref().unwrap(), &second);
        assert_eq!(d.run_id, Some(9));
        assert!(d.copy_path(0).is_none());
        assert!(d.copy_path(1).is_none());
    }
}
