//! Stable scopes and explicit aggregate membership shared by every representation.
use crate::data::{Category, Dataset};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Scope {
    pub root: usize,
    pub start: usize,
}
impl Scope {
    pub const ROOT: Self = Self { root: 0, start: 0 };
    pub fn folder(root: usize) -> Self {
        Self { root, start: 0 }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Choice {
    Node(usize),
    Group(Scope),
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub key: Choice,
    pub name: String,
    pub coverage: crate::data::Coverage,
    pub logical: u64,
    pub allocated: u64,
    pub count: usize,
    pub category: Category,
}
impl Entry {
    pub fn size(&self, allocated: bool) -> u64 {
        if allocated {
            self.allocated
        } else {
            self.logical
        }
    }
}
pub fn members(data: &Dataset, scope: Scope) -> Arc<Vec<usize>> {
    let all = data.children(scope.root, true);
    if scope.start == 0 {
        all
    } else {
        Arc::new(all.iter().skip(scope.start).copied().collect())
    }
}
pub fn entry(data: &Dataset, key: Choice, cs: bool) -> Entry {
    match key {
        Choice::Node(id) => {
            let n = &data.nodes[id];
            Entry {
                key,
                name: n.name.clone(),
                coverage: n.coverage,
                logical: n.logical,
                allocated: n.allocated,
                count: n.children.len(),
                category: n.category,
            }
        }
        Choice::Group(scope) => {
            let ids = data.children(scope.root, true);
            let start = scope.start.min(ids.len());
            let tail = &ids[start..];
            // Dataset directory totals equal the sum of their immutable children.
            // Visit the shorter side: visible prefixes are usually only 6–48 entries.
            let (logical, allocated) = if ids.is_empty() {
                (0, 0)
            } else if start <= tail.len() {
                let head = &ids[..start];
                let parent = &data.nodes[scope.root];
                (
                    parent.logical - head.iter().map(|&id| data.nodes[id].logical).sum::<u64>(),
                    parent.allocated - head.iter().map(|&id| data.nodes[id].allocated).sum::<u64>(),
                )
            } else {
                (
                    tail.iter().map(|&id| data.nodes[id].logical).sum(),
                    tail.iter().map(|&id| data.nodes[id].allocated).sum(),
                )
            };
            Entry {
                key,
                name: format!(
                    "{} {} {}",
                    if cs { "Dalších" } else { "Remaining" },
                    count(tail.len()),
                    if cs { "položek" } else { "items" }
                ),
                logical,
                allocated,
                count: tail.len(),
                category: data.nodes[scope.root].category,
                coverage: data.nodes[scope.root].coverage,
            }
        }
    }
}
pub fn contents(data: &Dataset, scope: Scope, limit: usize, cs: bool) -> Vec<Entry> {
    let all = data.children(scope.root, true);
    let tail = &all[scope.start.min(all.len())..];
    let limit = limit.max(1);
    let mut result: Vec<_> = tail
        .iter()
        .take(limit)
        .map(|&i| entry(data, Choice::Node(i), cs))
        .collect();
    if tail.len() > limit {
        result.push(entry(
            data,
            Choice::Group(Scope {
                root: scope.root,
                start: scope.start + limit,
            }),
            cs,
        ));
    }
    result
}
pub fn scope_entry(data: &Dataset, scope: Scope, cs: bool) -> Entry {
    entry(
        data,
        if scope.start == 0 {
            Choice::Node(scope.root)
        } else {
            Choice::Group(scope)
        },
        cs,
    )
}
pub fn plural<'a>(n: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    match n {
        1 => one,
        2..=4 => few,
        _ => many,
    }
}
pub fn count(v: usize) -> String {
    let raw = v.to_string();
    let mut out = String::new();
    for (i, ch) in raw.chars().enumerate() {
        if i > 0 && (raw.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}
#[derive(Debug)]
pub struct SearchResult {
    pub coverage: crate::data::Coverage,
    pub generation: u64,
    pub subtree: Vec<bool>,
    pub hits: Arc<Vec<usize>>,
    pub files: usize,
    pub logical: u64,
    pub allocated: u64,
}
/// Search runs off the UI thread. Parent propagation makes deep matches discoverable.
pub fn search(
    data: &Dataset,
    query: &str,
    category: Option<Category>,
    generation: u64,
    version: &AtomicU64,
) -> Option<SearchResult> {
    let query = query.trim().to_lowercase();
    let mut inherited = vec![false; data.nodes.len()];
    let mut subtree = vec![false; data.nodes.len()];
    let mut hits = Vec::new();
    let mut files = 0;
    let mut logical = 0;
    let mut allocated = 0;
    for (id, n) in data.nodes.iter().enumerate() {
        if id % 2048 == 0 && version.load(Ordering::Relaxed) != generation {
            return None;
        }
        let path_match = if query.contains('/') {
            if let Some(path) = &n.native_path {
                path.to_string_lossy().to_lowercase().contains(&query)
            } else {
                data.path(id)
                    .iter()
                    .map(|&i| data.nodes[i].name.to_lowercase())
                    .collect::<Vec<_>>()
                    .join("/")
                    .contains(&query)
            }
        } else {
            query.is_empty()
                || n.name.to_lowercase().contains(&query)
                || n.parent.is_some_and(|p| inherited[p])
        };
        inherited[id] = path_match;
        if id != 0 && path_match && category.is_none_or(|c| n.category == c) {
            hits.push(id);
            subtree[id] = true;
            if n.kind != crate::data::NodeKind::Directory {
                files += 1;
                logical += n.logical;
                allocated += n.allocated;
            }
        }
    }
    for id in (1..data.nodes.len()).rev() {
        if subtree[id]
            && let Some(parent) = data.nodes[id].parent
        {
            subtree[parent] = true;
        }
    }
    Some(SearchResult {
        // A query cannot prove absence or complete totals over an incomplete scan.
        coverage: data.nodes[0].coverage,
        generation,
        subtree,
        hits: Arc::new(hits),
        files,
        logical,
        allocated,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregate_boundaries_preserve_totals_for_both_metrics() {
        let d = Dataset::demo(10_000);
        let ids = d.children(2, true);
        for start in [
            0,
            1,
            6,
            48,
            ids.len() / 2,
            ids.len() - 1,
            ids.len(),
            ids.len() + 9,
        ] {
            let e = entry(&d, Choice::Group(Scope { root: 2, start }), true);
            let members = &ids[start.min(ids.len())..];
            assert_eq!(e.count, members.len());
            assert_eq!(
                e.logical,
                members.iter().map(|&i| d.nodes[i].logical).sum::<u64>()
            );
            assert_eq!(
                e.allocated,
                members.iter().map(|&i| d.nodes[i].allocated).sum::<u64>()
            );
        }
    }
    #[test]
    fn aggregate_is_an_exact_disjoint_tail_and_can_be_explored() {
        let d = Dataset::demo(5000);
        let scope = Scope::folder(2);
        let entries = contents(&d, scope, 8, true);
        let total = scope_entry(&d, scope, true);
        assert_eq!(
            entries.iter().map(|e| e.logical).sum::<u64>(),
            total.logical
        );
        assert_eq!(
            entries.iter().map(|e| e.allocated).sum::<u64>(),
            total.allocated
        );
        let Choice::Group(group) = entries.last().unwrap().key else {
            panic!("expected aggregate")
        };
        let members = members(&d, group);
        assert_eq!(members.len(), entries.last().unwrap().count);
        assert_eq!(
            members.iter().map(|&i| d.nodes[i].allocated).sum::<u64>(),
            entries.last().unwrap().allocated
        );
        assert!(
            members
                .iter()
                .all(|id| !entries[..8].iter().any(|e| e.key == Choice::Node(*id)))
        );
        assert_eq!(
            contents(&d, group, 5, false)
                .iter()
                .map(|e| e.logical)
                .sum::<u64>(),
            entries.last().unwrap().logical
        );
    }
    #[test]
    fn deep_name_and_path_matches_propagate_without_double_counting() {
        let d = Dataset::demo(500);
        let version = AtomicU64::new(7);
        let result = search(&d, "asset_000001", None, 7, &version).unwrap();
        assert_eq!(result.files, 1);
        let id = result.hits[0];
        for p in d.path(id) {
            assert!(result.subtree[p]);
        }
        assert_eq!(result.logical, d.nodes[id].logical);
        let path = d
            .path(id)
            .iter()
            .skip(1)
            .map(|&p| d.nodes[p].name.as_str())
            .collect::<Vec<_>>()
            .join("/");
        assert_eq!(search(&d, &path, None, 7, &version).unwrap().files, 1);
        assert!(
            search(&d, "asset_000001", Some(Category::Media), 7, &version)
                .unwrap()
                .hits
                .is_empty()
        );
        assert!(
            search(&d, "unfindable", None, 7, &version)
                .unwrap()
                .hits
                .is_empty()
        );
        assert!(search(&d, "", None, 6, &version).is_none());
    }
}
