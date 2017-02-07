extern crate froggy;

fn main() {
    let mut storage = froggy::Storage::new();
    {
        let mut s = storage.write();
        for &v in [5i32,7,4,6,7].iter() {
            s.create(v);
        }
    }
    let _ptr = {
        let s = storage.read();
        let i = s.iter().position(|v| *v == 4).unwrap();
        s.pin(i)
    };
    for value in storage.read().iter() {
        println!("Value {}", value);
    }
}
