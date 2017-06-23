extern crate froggy;

struct Fibbo {
    prev: Vec<froggy::Pointer<Fibbo>>,
    value: i32,
}

fn main() {
    let mut storage = froggy::Storage::new();
    // initialize the first two Fibbo numbers
    let mut first = storage.create(Fibbo {
        prev: Vec::new(),
        value: 1,
    });
    let mut second = storage.create(Fibbo {
        prev: vec![first.clone()],
        value: 0,
    });
    // initialize the other ones
    for _ in 0 .. 10 {
        let next = storage.create(Fibbo {
            prev: vec![first, second.clone()],
            value: 0,
        });
        first = second;
        second = next;
    }
    // compute them with look-back
    let mut cursor = storage.cursor();
    cursor.next().unwrap(); //skip first
    while let Some((left, mut item, _)) = cursor.next() {
        item.value = item.prev.iter().map(|prev|
            left.get(prev).unwrap().value
            ).sum();
        println!("{}", item.value);
    }
}
