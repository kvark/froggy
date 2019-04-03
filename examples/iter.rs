extern crate froggy;

fn main() {
    let mut storage: froggy::Storage<i32> = [5, 7, 4, 6, 7].iter().cloned().collect();
    println!("Initial array:");
    for value in storage.iter_all() {
        println!("Value {}", *value);
    }
    let ptr = {
        let item = storage.iter_all().find(|v| **v == 4).unwrap();
        storage.pin(&item)
    };
    storage[&ptr] = 350;
    println!("Array after change:");
    for value in storage.iter_all() {
        println!("Value {}", *value);
    }
}
