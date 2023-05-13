use std::sync::Arc;

use bwtree_rs::BwTree;
use shuttle::rand::{thread_rng, Rng};
use shuttle::thread;

#[test]
#[ignore = "not yet"]
fn test_disjoint_concurrent_inserts() {
    let tree = Arc::new(BwTree::new());
    shuttle::check_random(
        move || {
            let iterations = 10000;
            let t1_start = thread_rng().gen::<u64>();
            {
                let tree = tree.clone();
                thread::spawn(move || {
                    for i in 0..iterations {
                        let key = t1_start + i;
                        let value = thread_rng().gen::<u64>();
                        tree.insert(key, value);
                        assert_eq!(tree.get(key), Some(&value));
                    }
                });
            }
            let t2_start = t1_start + iterations;
            {
                let tree = tree.clone();
                thread::spawn(move || {
                    for i in 0..iterations {
                        let key = t2_start + i;
                        let value = thread_rng().gen::<u64>();
                        tree.insert(key, value);
                        assert_eq!(tree.get(key), Some(&value));
                    }
                });
            }
        },
        100,
    );
}
