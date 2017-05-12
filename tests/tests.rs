extern crate froggy;

use froggy::Storage;

#[test]
fn change_by_pointer() {
    let mut storage = Storage::new();
    {
        let mut s = storage.write();
        s.create(4 as i32);
    }
    let ptr = {
        let item = storage.iter().next().unwrap();
        storage.pin(&item)
    };
    assert_eq!(storage.write()[&ptr], 4);
    storage.write()[&ptr] = 350 as i32;
    assert_eq!(storage.write()[&ptr], 350);
}

#[test]
fn iterating() {
    let mut storage = Storage::new();
    {
        let mut s = storage.write();
        for &i in [5 as i32, 7, 4, 6, 7].iter() {
            s.create(i);
        }
    }
    assert_eq!(storage.iter().count(), 5);
    assert_eq!(*storage.iter().nth(0).unwrap(), 5);
    assert_eq!(*storage.iter().nth(1).unwrap(), 7);
    assert!(storage.iter().find(|v| **v == 4).is_some());
}

#[test]
fn iter_alive() {
    let mut storage = Storage::new();
    {
        let mut w = storage.write();
        for i in 0..5 {
            w.create(i * 3 as i32);
        }
    }
    assert_eq!(storage.write().iter().count(), 5);
    assert_eq!(storage.write().iter_alive().count(), 0);
    let ptr = {
        let item = storage.iter().nth(0).unwrap();
        storage.pin(&item)
    };
    assert_eq!(storage[&ptr], 0);
    assert_eq!(storage.write().iter().count(), 5);
    assert_eq!(storage.write().iter_alive().count(), 1);
}

#[test]
fn pointer_iter() {
    let mut storage = Storage::new();
    {
        let mut w = storage.write();
        for i in 0..5 {
            w.create(i as i32);
        }
    }
    let mut counter = 0;
    let mut write = storage.write();
    let mut iter_ptr = write.first();
    while let Some(ptr) = iter_ptr {
        assert_eq!(write[&ptr], counter);
        counter += 1;
        iter_ptr = write.advance(ptr);
    }
    assert_eq!(counter, 5);
}

#[test]
fn weak_upgrade_downgrade() {
    let mut storage = Storage::new();
    let ptr = storage.write().create(1 as i32);
    let _write = storage.write();
    let weak = ptr.downgrade();
    assert_eq!(weak.upgrade().is_ok(), true);
}

#[test]
fn weak_epoch() {
    let mut storage = Storage::new();
    let weak = {
        let node1 = storage.write().create(1 as i32);
        assert_eq!(storage.write().iter_alive().count(), 1);
        node1.downgrade()
    };
    assert_eq!(storage.write().iter_alive().count(), 0);
    assert_eq!(weak.upgrade(), Err(froggy::DeadComponentError));
    let _ptr = storage.write().create(1 as i32);
    assert_eq!(storage.write().iter_alive().count(), 1);
    assert_eq!(weak.upgrade(), Err(froggy::DeadComponentError));
}
