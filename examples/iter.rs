extern crate froggy;

fn main() {
    let mut storage = froggy::Storage::new();
    {
        let mut s = storage.write();
        for &v in [5 as i32, 7, 4, 6, 7].iter() {
            s.create(v);
        }
    }
    println!("Initial array:");
    for value in storage.read().iter() {
        println!("Value {}", *value);
    }
    {
        let ptr = {
            let r = storage.read();
            let item = r.iter().find(|v| **v == 4).unwrap();
            r.pin(&item)
        };
        let mut w = storage.write();
        w[&ptr] = 350 as i32;
    }
    println!("Array after change:");
    for value in storage.read().iter() {
        println!("Value {}", *value);
    }
}
