use std::sync::atomic::{AtomicPtr, Ordering};

/// A lock-free singly-linked list.
#[derive(Debug)]
pub struct LinkedList<T> {
    head: AtomicPtr<Node<T>>,
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        LinkedList {
            head: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn push_front(&self, value: T) {
        let new_node = Box::into_raw(Box::new(Node {
            value,
            next: AtomicPtr::new(std::ptr::null_mut()),
        }));

        loop {
            let head = self.head.load(Ordering::Acquire);

            unsafe {
                (*new_node).next.store(head, Ordering::Relaxed);
            }

            if self
                .head
                .compare_exchange_weak(head, new_node, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        let next = unsafe { self.head.load(Ordering::Relaxed).as_ref() };
        Iter { next }
    }
}

#[derive(Debug)]
struct Node<T> {
    value: T,
    next: AtomicPtr<Node<T>>,
}

pub struct Iter<'a, T> {
    next: Option<&'a Node<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.next {
            let next = unsafe { node.next.load(Ordering::Relaxed).as_ref() };
            self.next = next;
            Some(&node.value)
        } else {
            None
        }
    }
}
