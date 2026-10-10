//! Ordered maps and sets built one insert at a time.
//!
//! `collect()` into a `BTreeMap` or `BTreeSet` sorts the items first, which adds a copy of the sorting code to
//! the WebAssembly module for every type collected. Inserting one by one gives the same result (the last of
//! two equal keys wins in both) and costs next to nothing, since the type is inserted into anyway.

use std::collections::{BTreeMap, BTreeSet};

pub fn map_of<K: Ord, V>(items: impl IntoIterator<Item = (K, V)>) -> BTreeMap<K, V> {
    let mut map = BTreeMap::new();
    for (key, value) in items {
        map.insert(key, value);
    }
    map
}

pub fn set_of<T: Ord>(items: impl IntoIterator<Item = T>) -> BTreeSet<T> {
    let mut set = BTreeSet::new();
    for item in items {
        set.insert(item);
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_orders_like_collect_and_the_last_duplicate_wins() {
        let items = vec![(3, "c"), (1, "a"), (2, "b"), (1, "z")];
        assert_eq!(map_of(items.clone()), items.into_iter().collect());
        assert_eq!(map_of([(1, "a"), (1, "z")])[&1], "z");
        assert_eq!(set_of([3, 1, 2, 1]), [1, 2, 3].into_iter().collect());
    }
}
