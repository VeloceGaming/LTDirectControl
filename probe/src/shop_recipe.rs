//! Structural upgrade paths, independent of inventory and purchase orders.
//! Keep the graph and path counts rather than enumerating every combination:
//! a page can request any path directly, even in a heavily branching item mod.
use crate::shop::Item;

pub struct Tree {
    root: usize,
    parents: Vec<Vec<usize>>,
    counts: Vec<usize>,
}

impl Tree {
    pub fn new(cat: &[Item], root: usize) -> Self {
        let mut parents = vec![Vec::new(); cat.len()];
        for (from, item) in cat.iter().enumerate() {
            for &to in &item.next {
                if let Some(list) = parents.get_mut(to) {
                    if !list.contains(&from) {
                        list.push(from);
                    }
                }
            }
        }
        let mut counts = vec![0usize; cat.len()];
        let mut state = vec![0; cat.len()];
        fn visit(
            cat: &[Item],
            node: usize,
            parents: &mut [Vec<usize>],
            counts: &mut [usize],
            state: &mut [u8],
        ) {
            state[node] = 1;
            if cat[node].tier == 0 {
                parents[node].clear();
                counts[node] = 1;
            } else {
                let mut valid = Vec::new();
                for parent in parents[node].clone() {
                    // A malformed mod's cycle must not recurse forever, and
                    // a broken branch must not hide a valid alternative.
                    if state[parent] == 1 {
                        continue;
                    }
                    if state[parent] == 0 {
                        visit(cat, parent, parents, counts, state);
                    }
                    if counts[parent] != 0 {
                        valid.push(parent);
                        counts[node] = counts[node].saturating_add(counts[parent]);
                    }
                }
                parents[node] = valid;
            }
            state[node] = 2;
        }
        if root < cat.len() {
            visit(cat, root, &mut parents, &mut counts, &mut state);
        }
        Self {
            root,
            parents,
            counts,
        }
    }

    pub fn len(&self) -> usize {
        // An item with no valid base route still has its own detail tile.
        self.counts.get(self.root).map_or(0, |n| (*n).max(1))
    }

    pub fn path(&self, mut index: usize) -> Option<Vec<usize>> {
        if index >= self.len() {
            return None;
        }
        let mut node = self.root;
        let mut path = vec![node];
        while !self.parents[node].is_empty() {
            let parent = self.parents[node].iter().copied().find(|p| {
                if index < self.counts[*p] {
                    true
                } else {
                    index -= self.counts[*p];
                    false
                }
            })?;
            path.push(parent);
            node = parent;
        }
        path.reverse();
        Some(path)
    }

    /// First complete path ending with this chain. This restores a browsed
    /// branch and locates a purchase plan without discarding earlier parts.
    pub fn index_for_suffix(&self, suffix: &[usize]) -> Option<usize> {
        if suffix.last() != Some(&self.root) || self.len() == 0 {
            return None;
        }
        if suffix == [self.root] {
            return Some(0);
        }
        if self.counts.get(*suffix.first()?).copied()? == 0 {
            return None;
        }
        let mut index = 0usize;
        for edge in suffix.windows(2) {
            let parents = self.parents.get(edge[1])?;
            let position = parents.iter().position(|p| *p == edge[0])?;
            for parent in &parents[..position] {
                index = index.saturating_add(self.counts[*parent]);
            }
        }
        (index < self.len()).then_some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shop::{self, Step};

    fn item(key: &str, tier: usize, price: usize, next: &[usize]) -> Item {
        Item {
            key: key.into(),
            tier,
            price,
            next: next.to_vec(),
            category: String::new(),
            stats: Vec::new(),
            enabled: true,
        }
    }

    #[test]
    fn owned_component_does_not_remove_earlier_recipe_parts() {
        let cat = vec![
            item("book", 0, 250, &[1]),
            item("wand", 1, 250, &[2]),
            item("guise", 2, 500, &[]),
        ];
        let tree = Tree::new(&cat, 2);
        assert_eq!(tree.path(0), Some(vec![0, 1, 2]));
        assert_eq!(
            shop::plan(&cat, &[1], 2, false),
            Some(vec![Step::Upgrade { slot: 0, item: 2 }])
        );
        assert_eq!(tree.index_for_suffix(&[1, 2]), Some(0));
        // Even owning the completed item leaves the complete recipe visible.
        assert!(shop::plan(&cat, &[2], 2, false).is_none());
        assert_eq!(tree.path(0), Some(vec![0, 1, 2]));
    }

    #[test]
    fn alternative_paths_remain_visible_when_buyer_chooses_cheaper_branch() {
        let cat = vec![
            item("sword", 0, 250, &[1]),
            item("pickaxe", 1, 500, &[2, 3]),
            item("scepter", 2, 800, &[4]),
            item("axe", 2, 500, &[4]),
            item("blade", 3, 750, &[]),
        ];
        let tree = Tree::new(&cat, 4);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.path(0), Some(vec![0, 1, 2, 4]));
        assert_eq!(tree.path(1), Some(vec![0, 1, 3, 4]));
        assert_eq!(tree.index_for_suffix(&[2, 4]), Some(0));
        let plan = shop::plan(&cat, &[], 4, false).unwrap();
        let steps: Vec<_> = plan.iter().map(|s| s.item()).collect();
        assert_eq!(steps, vec![0, 1, 3, 4]);
        assert_eq!(tree.index_for_suffix(&steps), Some(1));
        assert_eq!(
            plan.iter().map(|s| cat[s.item()].price).sum::<usize>(),
            2000
        );
        let owned_plan = shop::plan(&cat, &[2], 4, true).unwrap();
        assert_eq!(owned_plan, vec![Step::Upgrade { slot: 0, item: 4 }]);
        assert_eq!(tree.index_for_suffix(&[2, 4]), Some(0));
    }

    #[test]
    fn long_chains_and_every_alternative_are_addressable_without_truncation() {
        let mut cat: Vec<_> = (0..9).map(|i| item("part", i, 1, &[i + 1])).collect();
        cat[8].next = vec![12];
        for i in 9..12 {
            cat.push(item("other", 0, i, &[12]));
        }
        cat.push(item("final", 9, 1, &[]));
        let tree = Tree::new(&cat, 12);
        assert_eq!(tree.len(), 4);
        assert_eq!(tree.path(0), Some((0..9).chain([12]).collect()));
        for i in 1..4 {
            assert_eq!(tree.path(i), Some(vec![i + 8, 12]));
        }
        assert_eq!(tree.path(4), None);
    }

    #[test]
    fn many_combinations_need_only_a_graph_and_counts() {
        let mut cat = vec![item("base", 0, 1, &[1, 2])];
        for tier in 1..=30 {
            let next = if tier == 30 {
                vec![61]
            } else {
                vec![tier * 2 + 1, tier * 2 + 2]
            };
            cat.push(item("left", tier, 1, &next));
            cat.push(item("right", tier, 1, &next));
        }
        cat.push(item("final", 31, 1, &[]));
        let tree = Tree::new(&cat, 61);
        assert_eq!(tree.len(), 1 << 30);
        let last = tree.path(tree.len() - 1).unwrap();
        assert_eq!(last.len(), 32);
        assert_eq!(tree.index_for_suffix(&last), Some(tree.len() - 1));
        assert_eq!(tree.index_for_suffix(&[60, 61]), Some(1 << 29));
    }

    #[test]
    fn cycles_duplicate_edges_and_unknown_indices_do_not_hide_valid_paths() {
        let cat = vec![
            item("base", 0, 1, &[1, 1, 999]),
            item("middle", 1, 1, &[2]),
            item("final", 2, 1, &[1]),
            item("orphan", 4, 1, &[]),
        ];
        let tree = Tree::new(&cat, 2);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.path(0), Some(vec![0, 1, 2]));
        assert_eq!(tree.index_for_suffix(&[0, 2]), None);
        assert_eq!(Tree::new(&cat, 3).path(0), Some(vec![3]));
        assert_eq!(Tree::new(&cat, 999).path(0), None);
    }
}
