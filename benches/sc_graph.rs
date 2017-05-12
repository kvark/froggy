#![feature(test)]

extern crate test;
extern crate froggy;

use test::Bencher;
use froggy::{Pointer, Storage};

// Entities with a Postion and Velocity component
pub const N_POS_VEL: usize = 1000;
// Entities with a Position component only
pub const N_POS: usize = 9000;

struct Position {
    pub x: f32,
    pub y: f32,
}

struct Velocity {
    pub dx: f32,
    pub dy: f32,
    pub writes: Pointer<Position>,
}

struct Movement {
	pub vel_comp: Vec<Pointer<Velocity>>,
}

struct World {
    pub pos: Storage<Position>,
    pub vel: Storage<Velocity>,
    pub movement: Movement,
}

fn build() -> World {

    let mut world = World {
        pos: Storage::with_capacity(N_POS_VEL + N_POS),
        vel: Storage::with_capacity(N_POS_VEL),
        movement: Movement{ vel_comp: Vec::new() },
    };


    {
        for fx in 1 .. (N_POS_VEL + N_POS + 1) {
            let pos_ptr = world.pos.create(Position { x: 0.0, y: 0.0 });

    	    // Every 10th Position has a Veloctiy
    	    // This way we test for a 'more real' scenario
    	    if fx % 10 == 0 {
    	    	let v = Velocity { dx: 0.0, dy: 0.0, writes: pos_ptr};
    	        world.movement.vel_comp.push(world.vel.create(v));
    	    }
        }
    }

    world
}

#[bench]
fn bench_build(b: &mut Bencher) {
    b.iter(|| build());
}

#[bench]
fn bench_update(b: &mut Bencher) {
    let mut world = build();

    b.iter(|| {
        for value in world.vel.iter() {
            let mut p = &mut world.pos[&value.writes];
            p.x += value.dx;
            p.y += value.dy;
        }
    });
}
