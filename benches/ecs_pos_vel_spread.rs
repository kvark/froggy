use criterion::{criterion_group, criterion_main, Criterion};
use froggy::{Pointer, Storage};

mod bench_setup;
use bench_setup::{Position, N_POS, N_POS_VEL};

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
        let pos_spread = (N_POS + N_POS_VEL) / N_POS_VEL;

        for fx in 1..(N_POS_VEL + N_POS + 1) {
            world.entities.push(Entity {
                pos: world.pos.create(Position { x: 0.0, y: 0.0 }),
                vel: None,
            });
            if fx % pos_spread == 0 {
                world.entities.push(Entity {
                    pos: world.pos.create(Position { x: 0.0, y: 0.0 }),
                    vel: Some(world.vel.create(Velocity { dx: 0.0, dy: 0.0 })),
                });
            }
        }
    }

    world
}

fn bench_build(c: &mut Criterion) {
    c.bench_function("build-ecs-spread", |b| b.iter(|| build()));
}

fn bench_update(c: &mut Criterion) {
    let mut world = build();

    c.bench_function("update-ecs-spread", move |b| {
        b.iter(|| {
            for e in &world.entities {
                if let Some(ref vel) = e.vel {
                    let mut p = &mut world.pos[&e.pos];
                    let v = &world.vel[vel];
                    p.x += v.dx;
                    p.y += v.dy;
                }
            }
        })
    });
}

criterion_group!(benches, bench_build, bench_update);
criterion_main!(benches);
