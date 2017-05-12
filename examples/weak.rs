extern crate froggy;
use froggy::{Storage, WeakPointer};

struct Node {
    value: String,
    next: Option<WeakPointer<Node>>,
}

impl Drop for Node {
    fn drop(&mut self) {
        println!("{} destroyed", &self.value);
    }
}

fn main() {
    let mut storage = Storage::new();
    let node1 = storage.write().create(Node {
        value: "Node 1".to_string(),
        next: None,
    });
    let node2 = storage.write().create(Node {
        value: "Node 2".to_string(),
        next: None,
    });

    storage.write()[&node1].next = Some(node2.downgrade());
    storage.write()[&node2].next = Some(node1.downgrade());

    for node in storage.iter() {
        let value = node.next.as_ref().map_or("None".into(), |ref next| {
            let ptr = next.upgrade().unwrap();
            storage[&ptr].value.clone()
        });
        println!("{} has `next` field with value {}", node.value, value);
    }
}
