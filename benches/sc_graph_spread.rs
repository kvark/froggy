#![feature(test)]

extern crate test;
extern crate froggy;

use test::Bencher;
use froggy::{Pointer, Storage};

mod bench_setup;
use bench_setup::{Position, Velocity, N_POS_VEL, N_POS};

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
        let pos_spread = (N_POS + N_POS_VEL) / N_POS_VEL;

        for fx in 1 .. (N_POS_VEL + N_POS + 1) {
            let pos_ptr = world.pos.create(Position { x: 0.0, y: 0.0 });

    	    if fx % pos_spread == 0 {
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
        for vel in world.vel.iter() {
            let mut p = &mut world.pos[&vel.writes];
            p.x += vel.dx;
            p.y += vel.dy;
        }
    });
}
