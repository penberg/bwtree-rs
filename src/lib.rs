mod linked_list;

use crate::linked_list::LinkedList;
use std::fmt::Debug;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

const FIRST_LEAF_NODE_ID: usize = 2;

pub trait KeyType: Ord {
    const MINIMUM: Self;
}

impl KeyType for u64 {
    const MINIMUM: Self = u64::MIN;
}

pub type NodeID = usize;

/// Bw-Tree is a latch-free index for modern multicore machines.
#[derive(Default)]
pub struct BwTree<K, V>
where
    K: KeyType + Debug,
    V: Clone + Debug,
{
    root_id: usize,
    /// Mapping table from logical node IDs to physical pointers.
    mapping_table: MappingTable<K, V>,
    /// The next unused node ID in the `mapping_table`.
    next_unused_node_id: AtomicUsize,
}

impl<K, V> BwTree<K, V>
where
    K: KeyType + Debug,
    V: Clone + Debug,
{
    pub fn new() -> Self {
        let ret: BwTree<K, V> = BwTree {
            root_id: 1,
            mapping_table: MappingTable::new(),
            next_unused_node_id: AtomicUsize::new(1),
        };

        // The Bw-Tree initially consists of two nodes: an empty leaf node
        // and an inner node that contains the empty leaf node
        let root_id = ret.get_next_node_id();
        assert_eq!(1, root_id);
        let first_leaf_id = ret.get_next_node_id();
        assert_eq!(FIRST_LEAF_NODE_ID, first_leaf_id);

        let left_most_leaf = Node::Leaf(LeafNode::new());
        let mut root = InnerNode::new();
        root.insert(KeyType::MINIMUM, first_leaf_id);

        ret.mapping_table.insert(root_id, Node::Inner(root));
        ret.mapping_table.insert(first_leaf_id, left_most_leaf);

        ret
    }

    fn get_next_node_id(&self) -> NodeID {
        // TODO: recycle deleted node IDs
        self.next_unused_node_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn insert(&self, key: K, value: V) -> bool {
        let root = self.mapping_table.get(self.root_id);
        match root {
            Node::Inner(_) => {
                let delta = DeltaNode::new();
                delta.insert(key, value);
                let delta = Node::Delta(delta);
                self.mapping_table.insert(self.root_id, delta);
            }
            Node::Delta(delta) => {
                delta.insert(key, value);
            }
            Node::Leaf(_) => todo!(),
        }
        true
    }

    pub fn get(&self, key: K) -> Option<&V> {
        let root = self.mapping_table.get(self.root_id);
        root.get(&key)
    }
}

const MAPPING_TABLE_SIZE: usize = 1 << 20;

/// Mapping from logical node IDs to physical pointers.
#[derive(Default)]
pub struct MappingTable<K: Ord, V: Clone> {
    /// The mapping table.
    entries: Vec<AtomicPtr<Node<K, V>>>,
}

impl<K: Ord, V: Clone> MappingTable<K, V> {
    pub fn new() -> Self {
        let mut entries = Vec::default();
        entries.resize_with(MAPPING_TABLE_SIZE, AtomicPtr::default);
        MappingTable { entries }
    }

    fn get(&self, id: usize) -> &Node<K, V> {
        assert!(id < MAPPING_TABLE_SIZE);
        let entry = self.entries[id].load(Ordering::Acquire);
        assert!(!entry.is_null());
        unsafe { &*entry }
    }

    fn insert(&self, id: usize, node: Node<K, V>) -> bool {
        assert!(id < MAPPING_TABLE_SIZE);
        let entry = &self.entries[id];
        let old = entry.load(Ordering::Acquire);
        let new = Box::leak(Box::new(node));
        match entry.compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_old) => {
                // TODO: deferred delete of '_old'
                true
            }
            Err(new) => {
                std::mem::drop(Box::from(new));
                false
            }
        }
    }
}

#[derive(Debug)]
enum Node<K, V> {
    Inner(InnerNode<K>),
    Delta(DeltaNode<K, V>),
    Leaf(LeafNode<K, V>),
}

impl<K, V> Node<K, V>
where
    K: KeyType,
{
    fn get(&self, key: &K) -> Option<&V> {
        match self {
            Node::Inner(_) => todo!(),
            Node::Delta(node) => node.get(key),
            Node::Leaf(node) => node.get(key),
        }
    }
}

#[derive(Debug)]
struct InnerNode<K> {
    /// The key ranges stored in the children.
    keys: Vec<K>,
    /// Pointers to the children.
    children: Vec<NodeID>,
}

impl<K> InnerNode<K> {
    fn new() -> Self {
        InnerNode {
            keys: Vec::new(),
            children: Vec::new(),
        }
    }

    fn insert(&mut self, key: K, node_id: NodeID) {
        self.keys.push(key);
        self.children.push(node_id);
    }
}

#[derive(Debug)]
struct DeltaNode<K, V> {
    records: LinkedList<DeltaRecord<K, V>>,
}

impl<K, V> DeltaNode<K, V>
where
    K: KeyType,
{
    fn new() -> Self {
        DeltaNode {
            records: LinkedList::new(),
        }
    }

    fn insert(&self, key: K, value: V) {
        self.records.push_front(DeltaRecord::Insert(key, value));
    }

    fn get(&self, key: &K) -> Option<&V> {
        for ref record in self.records.iter() {
            match record {
                DeltaRecord::Insert(k, v) => {
                    if key == k {
                        return Some(v);
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug)]
enum DeltaRecord<K, V> {
    Insert(K, V),
}

#[derive(Debug)]
struct LeafNode<K, V> {
    /// The number of keys stored in the node.
    count: usize,
    /// The key ranges stored in the children.
    keys: Vec<K>,
    /// The values stored in the node.
    values: Vec<V>,
}

impl<K, V> LeafNode<K, V>
where
    K: KeyType,
{
    fn new() -> Self {
        LeafNode {
            count: 0,
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    fn get(&self, key: &K) -> Option<&V> {
        for i in 0..self.count {
            if key == &self.keys[i] {
                return Some(&self.values[i]);
            }
        }
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let tree = BwTree::new();
        assert!(tree.insert(1, "A"));
        assert_eq!(tree.get(1), Some(&"A"));
        assert!(tree.insert(2, "B"));
        assert_eq!(tree.get(2), Some(&"B"));
        assert!(tree.insert(3, "C"));
        assert_eq!(tree.get(3), Some(&"C"));
        assert!(tree.insert(4, "D"));
        assert_eq!(tree.get(4), Some(&"D"));
    }

    #[test]
    fn test_insert_retains_existing_entries() {
        // The Bw-Tree stores insertions into a delta chain. Let's make sure
        // that `insert()` doesn't lose existing entries.
        let tree = BwTree::new();
        assert!(tree.insert(1, "A"));
        assert_eq!(tree.get(1), Some(&"A"));
        assert!(tree.insert(2, "B"));
        assert_eq!(tree.get(1), Some(&"A"));
        assert!(tree.insert(2, "B"));
    }
}
