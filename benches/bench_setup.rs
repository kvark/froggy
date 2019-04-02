use froggy::Pointer;

/// Entities with velocity and position component.
pub const N_POS_VEL: usize = 5_000;

/// Entities with position component only.
pub const N_POS: usize = 15_000;

pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[allow(dead_code)]
pub struct Velocity {
    pub dx: f32,
    pub dy: f32,
    pub writes: Pointer<Position>,
}
