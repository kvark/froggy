use froggy::{Pointer, Storage, WeakPointer};

#[test]
fn sizes() {
    use std::mem::size_of;
    assert_eq!(size_of::<Pointer<()>>(), 16);
    assert_eq!(size_of::<Option<Pointer<()>>>(), 16);
}

#[test]
fn change_by_pointer() {
    let mut storage = Storage::new();
    storage.create(4 as i32);
    let ptr = {
        let item = storage.iter().next().unwrap();
        storage.pin(&item)
    };
    assert_eq!(storage[&ptr], 4);
    storage[&ptr] = 350 as i32;
    assert_eq!(storage[&ptr], 350);
}

#[test]
fn iterating() {
    let storage: Storage<_> = [5 as i32, 7, 4, 6, 7].iter().cloned().collect();
    assert_eq!(storage.iter_all().count(), 5);
    assert_eq!(*storage.iter_all().nth(0).unwrap(), 5);
    assert_eq!(*storage.iter_all().nth(1).unwrap(), 7);
    assert!(storage.iter_all().any(|v| *v == 4));
}

#[test]
fn iter_zombies() {
    let storage: Storage<_> = (0..5).map(|i| i * 3 as i32).collect();
    assert_eq!(storage.iter().count(), 0);
    assert_eq!(storage.iter_all().count(), 5);
}

#[test]
fn weak_upgrade_downgrade() {
    let mut storage = Storage::new();
    let ptr = storage.create(1 as i32);
    let weak = ptr.downgrade();
    assert_eq!(weak.upgrade().is_ok(), true);
}

#[test]
fn weak_epoch() {
    let mut storage = Storage::new();
    let weak = {
        let node1 = storage.create(1 as i32);
        assert_eq!(storage.iter().count(), 1);
        node1.downgrade()
    };
    storage.sync_pending();
    assert_eq!(storage.iter_mut().count(), 0);
    assert_eq!(weak.upgrade(), Err(froggy::DeadComponentError));
    let _ptr = storage.create(1 as i32);
    storage.sync_pending();
    assert_eq!(storage.iter_mut().count(), 1);
    assert_eq!(weak.upgrade(), Err(froggy::DeadComponentError));
}

#[test]
fn cursor() {
    let data = vec![5 as i32, 7, 4, 6, 7];
    let mut storage: Storage<_> = data.iter().cloned().collect();

    let mut cursor = storage.cursor();
    let mut iter = data.iter();
    while let Some((_, item, _)) = cursor.next() {
        assert_eq!(iter.next(), Some(&*item));
        let _ptr = item.pin();
    }

    while let Some((_, item, _)) = cursor.prev() {
        assert_eq!(iter.next_back(), Some(&*item));
    }
}

#[test]
fn partial_ord() {
    use std::cmp::Ordering;
    let mut storage = Storage::new();
    let a = storage.create(1u32);
    let b = storage.create(1u32);
    let c = storage.create(1u32);
    assert_eq!(a.partial_cmp(&b), Some(Ordering::Less));
    assert_eq!(c.partial_cmp(&b), Some(Ordering::Greater));
    let a2 = storage.pin(&storage.iter().next().unwrap());
    assert_eq!(a.partial_cmp(&a2), Some(Ordering::Equal));
    let mut storage2 = Storage::new();
    let a3 = storage2.create(1u32);
    // Different storages
    assert_eq!(a.partial_cmp(&a3), None);
}

#[test]
fn slice() {
    let mut storage = Storage::new();
    let a = storage.create(1u32);
    let b = storage.create(2u32);
    let c = storage.create(3u32);
    let (left, mid, right) = storage.split(&b);
    assert_eq!(*mid, 2);
    assert_eq!(left.get(&a), Some(&1));
    assert_eq!(right.get(&c), Some(&3));
    assert!(left.get(&b).is_none() && left.get(&c).is_none());
    assert!(right.get(&a).is_none() && right.get(&b).is_none());
}

#[test]
fn storage_default() {
    let mut storage = Storage::default();
    storage.create(1u32);
}

#[test]
fn pointer_eq() {
    let mut storage = Storage::new();
    storage.create(1u32);
    storage.create(2u32);
    let ptr1 = storage.pin(&storage.iter().next().unwrap());
    let ptr2 = storage.pin(&storage.iter().nth(1).unwrap());
    let ptr3 = storage.pin(&storage.iter().nth(1).unwrap());
    // PartialEq
    assert_eq!(ptr2, ptr3);
    assert_ne!(ptr1, ptr2);
    assert_ne!(ptr1, ptr3);
    // Reflexive
    assert_eq!(ptr1, ptr1);
    assert_eq!(ptr2, ptr2.clone());
}

#[test]
fn weak_pointer_eq() {
    let mut storage = Storage::new();
    storage.create(1u32);
    storage.create(2u32);
    let weak_ptr1 = storage.pin(&storage.iter().next().unwrap()).downgrade();
    let ptr2 = storage.pin(&storage.iter().nth(1).unwrap());
    let weak_ptr2 = ptr2.downgrade();
    let weak_ptr3 = ptr2.downgrade();
    // PartialEq
    assert_eq!(weak_ptr2, weak_ptr3);
    assert_ne!(weak_ptr1, weak_ptr2);
    assert_ne!(weak_ptr1, weak_ptr3);
    // Reflexive
    assert_eq!(weak_ptr1, weak_ptr1);
    assert_eq!(weak_ptr2, weak_ptr2.clone());
    assert_eq!(weak_ptr3.upgrade().unwrap(), weak_ptr2.upgrade().unwrap());
}

#[test]
fn test_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Storage<i32>>();
    assert_send::<Pointer<i32>>();
    assert_send::<WeakPointer<i32>>();
    assert_send::<froggy::DeadComponentError>();
}

#[test]
fn test_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<Storage<i32>>();
    assert_sync::<Pointer<i32>>();
    assert_sync::<WeakPointer<i32>>();
    assert_sync::<froggy::DeadComponentError>();
}

#[test]
fn test_hash() {
    use std::collections::HashMap;
    let mut hash_map = HashMap::new();
    let mut storage = Storage::new();
    let ptr = storage.create(1u8);
    hash_map.insert(ptr.clone(), 23u8);
    assert_eq!(hash_map.get(&ptr), Some(&23u8));
}
