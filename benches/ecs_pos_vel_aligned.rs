#![feature(test)]

extern crate test;
extern crate froggy;

use test::Bencher;
use froggy::{Pointer, Storage};

mod bench_setup;
use bench_setup::{Position, N_POS_VEL, N_POS};

// Since component linking is not used in this bench,
// it has a custom Velocity component
struct Velocity {
    pub dx: f32,
    pub dy: f32,
}

struct Entity {
    pos: Pointer<Position>,
    vel: Option<Pointer<Velocity>>,
}

struct World {
    pos: Storage<Position>,
    vel: Storage<Velocity>,
    entities: Vec<Entity>,
}

fn build() -> World {
    let mut world = World {
        pos: Storage::with_capacity(N_POS_VEL + N_POS),
        vel: Storage::with_capacity(N_POS_VEL),
        entities: Vec::with_capacity(N_POS_VEL + N_POS),
    };

    // setup entities
    {
        for _ in 0 .. N_POS_VEL {
            world.entities.push(Entity {
                pos: world.pos.create(Position { x: 0.0, y: 0.0 }),
                vel: Some(world.vel.create(Velocity { dx: 0.0, dy: 0.0 })),
            });
        }
        for _ in 0 .. N_POS {
            world.entities.push(Entity {
                pos: world.pos.create(Position { x: 0.0, y: 0.0 }),
                vel: None,
            });
        }
    }

    world
}

#[bench]
fn bench_build(b: &mut Bencher) {
    b.iter(build);
}

#[bench]
fn bench_update(b: &mut Bencher) {
    let mut world = build();

    b.iter(|| {
        for e in &world.entities {
            if let Some(ref vel) = e.vel {
                let mut p = &mut world.pos[&e.pos];
                let v = &world.vel[vel];
                p.x += v.dx;
                p.y += v.dy;
            }
        }
    });
}
