use std::collections::BTreeMap;
use std::path::PathBuf;

use ivygrep::merkle::MerkleSnapshot;
use proptest::prelude::*;

fn map_strategy() -> impl Strategy<Value = BTreeMap<String, String>> {
    prop::collection::btree_map("[a-z]{1,6}\\.(rs|py|ts)", "[0-9a-f]{8}", 0..20)
}

proptest! {
    #[test]
    fn merkle_diff_matches_manual_expectation(
        old_files in map_strategy(),
        new_files in map_strategy(),
    ) {
        let old_root = format!("old_{}", old_files.len());
        let new_root = if old_files == new_files {
            old_root.clone()
        } else {
            format!("new_{}", new_files.len())
        };

        let old_snapshot = MerkleSnapshot { root_hash: old_root, files: old_files.clone() };
        let new_snapshot = MerkleSnapshot { root_hash: new_root, files: new_files.clone() };

        let mut expected_add_or_modified = Vec::new();
        let mut expected_deleted = Vec::new();

        for (path, new_hash) in &new_files {
            match old_files.get(path) {
                Some(old_hash) if old_hash == new_hash => {}
                _ => expected_add_or_modified.push((PathBuf::from(path), new_hash.ends_with("-1"))),
            }
        }

        for path in old_files.keys() {
            if !new_files.contains_key(path) {
                expected_deleted.push(PathBuf::from(path));
            }
        }

        expected_add_or_modified.sort();
        expected_deleted.sort();

        let mut diff = old_snapshot.diff(&new_snapshot);
        diff.added_or_modified.sort();
        diff.deleted.sort();

        prop_assert_eq!(diff.added_or_modified, expected_add_or_modified);
        prop_assert_eq!(diff.deleted, expected_deleted);
    }
}
