extern crate froggy;

fn main() {
    let mut storage = froggy::Storage::new();
    {
    	let mut s = storage.write();
    	s.create(4i32);
    }
    for value in storage.read().iter() {
        println!("Value {}", value);
    }
}
