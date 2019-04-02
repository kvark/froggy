use criterion::{criterion_group, criterion_main, Criterion};
use froggy::{Pointer, Storage};

mod bench_setup;
use bench_setup::{Position, Velocity, N_POS, N_POS_VEL};

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
        movement: Movement {
            vel_comp: Vec::new(),
        },
    };

    {
        for _ in 0..N_POS_VEL {
            let pos_ptr = world.pos.create(Position { x: 0.0, y: 0.0 });
            let v = Velocity {
                dx: 0.0,
                dy: 0.0,
                writes: pos_ptr,
            };
            world.movement.vel_comp.push(world.vel.create(v));
        }

        for _ in 0..N_POS {
            world.pos.create(Position { x: 0.0, y: 0.0 });
        }
    }

    world
}

fn bench_build(c: &mut Criterion) {
    c.bench_function("build-graph-aligned", |b| b.iter(|| build()));
}

fn bench_update(c: &mut Criterion) {
    let mut world = build();

    c.bench_function("update-graph-aligned", move |b| {
        b.iter(|| {
            for vel in world.vel.iter() {
                let mut p = &mut world.pos[&vel.writes];
                p.x += vel.dx;
                p.y += vel.dy;
            }
        })
    });
}

criterion_group!(benches, bench_build, bench_update);
criterion_main!(benches);
