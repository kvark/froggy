extern crate cgmath;
extern crate froggy;
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

use std::{mem, time};
use cgmath::{Angle, EuclideanSpace, One, Rotation3, Transform, Zero};
use gfx::traits::{Device, Factory, FactoryExt};


pub type ColorFormat = gfx::format::Srgba8;
pub type DepthFormat = gfx::format::DepthStencil;

gfx_defines!{
    vertex Vertex {
        pos: [f32; 4] = "a_Pos",
        normal: [i8; 4] = "a_Normal",
    }

    vertex Instance {
        offset_scale: [f32; 4] = "a_OffsetScale",
        rotation: [f32; 4] = "a_Rotation",
        color: [f32; 4] = "a_Color",
    }

    constant Globals {
        projection: [[f32; 4]; 4] = "u_Projection",
        camera_pos: [f32; 4] = "u_CameraPos",
        light_pos: [f32; 4] = "u_LightPos",
        light_color: [f32; 4] = "u_LightColor",
    }

    pipeline pipe {
        vert_buf: gfx::VertexBuffer<Vertex> = (),
        inst_buf: gfx::InstanceBuffer<Instance> = (),
        globals: gfx::ConstantBuffer<Globals> = "b_Globals",
        out_color: gfx::RenderTarget<ColorFormat> = "Target0",
        out_depth: gfx::DepthTarget<DepthFormat> = gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

fn vertex(x: i8, y: i8, z: i8, nx: i8, ny: i8, nz: i8) -> Vertex {
    Vertex {
        pos: [x as f32, y as f32, z as f32, 1.0],
        normal: [nx, ny, nz, 0],
    }
}


fn create_geometry<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F)
                   -> (gfx::handle::Buffer<R, Vertex>, gfx::Slice<R>)
{
    let vertices = [
        // bottom
        vertex(1, 1, 0, 0, 0, -1),
        vertex(1, -1, 0, 0, 0, -1),
        vertex(-1, -1, 0, 0, 0, -1),
        vertex(-1, 1, 0, 0, 0, -1),
        // top
        vertex(1, 1, 2, 0, 0, 1),
        vertex(1, -1, 2, 0, 0, 1),
        vertex(-1, -1, 2, 0, 0, 1),
        vertex(-1, 1, 2, 0, 0, 1),
        // left
        vertex(-1, 1, 2, -1, 0, 0),
        vertex(-1, 1, 0, -1, 0, 0),
        vertex(-1, -1, 0, -1, 0, 0),
        vertex(-1, -1, 2, -1, 0, 0),
        // right
        vertex(1, 1, 2, 1, 0, 0),
        vertex(1, 1, 0, 1, 0, 0),
        vertex(1, -1, 0, 1, 0, 0),
        vertex(1, -1, 2, 1, 0, 0),
        // front
        vertex(1, -1, 2, 0, -1, 0),
        vertex(1, -1, 0, 0, -1, 0),
        vertex(-1, -1, 0, 0, -1, 0),
        vertex(-1, -1, 2, 0, -1, 0),
        // back
        vertex(1, 1, 2, 0, 1, 0),
        vertex(1, 1, 0, 0, 1, 0),
        vertex(-1, 1, 0, 0, 1, 0),
        vertex(-1, 1, 2, 0, 1, 0),
    ];

    let indices = [
        0u16, 1, 2, 0, 2, 3,
        4, 5, 6, 4, 6, 7,
        8, 9, 10, 8, 10, 11,
        12, 13, 14, 12, 14, 15,
        16, 17, 18, 16, 18, 19,
        20, 21, 22, 20, 22, 23,
    ];

    factory.create_vertex_buffer_with_slice(&vertices, &indices[..])
}


type Space = cgmath::Decomposed<cgmath::Vector3<f32>, cgmath::Quaternion<f32>>;

struct Level {
    speed: f32,
}

struct Material {
    color: [f32; 4],
}

struct Node {
    local: Space,
    world: Space,
    parent: Option<froggy::Pointer<Node>>,
}

struct Cube {
    node: froggy::Pointer<Node>,
    material: froggy::Pointer<Material>,
    level: froggy::Pointer<Level>,
}

fn create_cubes(mut nodes: froggy::WriteLock<Node>,
                materials: froggy::ReadLock<Material>,
                levels: froggy::ReadLock<Level>)
                -> Vec<Cube>
{
    let mut list = vec![
        Cube {
            node: nodes.create(Node {
                local: Space {
                    disp: cgmath::Vector3::zero(),
                    rot: cgmath::Quaternion::one(),
                    scale: 2.0,
                },
                world: Space::one(),
                parent: None,
            }),
            material: materials.pin(0).unwrap(),
            level: levels.pin(0).unwrap(),
        }
    ];
    struct Stack {
        parent: froggy::Pointer<Node>,
        level_id: usize,
    }
    let mut stack = vec![
        Stack {
            parent: list[0].node.clone(),
            level_id: 0,
        }
    ];

    let axis = [cgmath::Vector3::unit_z(),
                cgmath::Vector3::unit_x(), -cgmath::Vector3::unit_x(),
                cgmath::Vector3::unit_y(), -cgmath::Vector3::unit_y()];
    let children: Vec<_> = axis.iter().map(|&axe| {
        Space {
            disp: cgmath::vec3(0.0, 0.0, 1.0),
            rot: cgmath::Quaternion::from_axis_angle(axe, cgmath::Rad::turn_div_4()),
            scale: 1.0,
        }.concat(&Space {
            disp: cgmath::vec3(0.0, 0.0, 1.0),
            rot: cgmath::Quaternion::one(),
            scale: 0.4,
        })
    }).collect();

    while let Some(next) = stack.pop() {
        //HACK: materials are indexed the same way as levels
        // it's fine for demostration purposes
        let material = match materials.pin(next.level_id + 1) {
            Some(material) => material,
            None => continue,
        };
        let level = match levels.pin(next.level_id + 1) {
            Some(level) => level,
            None => continue,
        };
        for child in children.iter() {
            let cube = Cube {
                node: nodes.create(Node {
                    local: child.clone(),
                    world: Space::one(),
                    parent: Some(next.parent.clone()),
                }),
                material: material.clone(),
                level: level.clone(),
            };
            stack.push(Stack {
                parent: cube.node.clone(),
                level_id: next.level_id + 1,
            });
            list.push(cube);
        }
    }

    list
}

fn make_globals(camera_pos: cgmath::Point3<f32>, aspect: f32) -> Globals {
    let mx_proj = {
        let fovy = cgmath::Deg(60.0);
        let perspective = cgmath::perspective(fovy, aspect, 1.0, 100.0);
        let focus = cgmath::Point3::new(0.0, 0.0, 3.0);
        let view = cgmath::Matrix4::look_at(
            camera_pos, focus, cgmath::Vector3::unit_z());
        perspective * view
    };
    Globals {
        projection: mx_proj.into(),
        camera_pos: camera_pos.to_vec().extend(1.0).into(),
        light_pos: [0.0, -10.0, 10.0, 1.0],
        light_color: [1.0, 1.0, 1.0, 1.0],
    }
}

const COLORS: [[f32; 4]; 6] = [
    [1.0, 1.0, 0.5, 1.0],
    [0.5, 0.5, 1.0, 1.0],
    [0.5, 1.0, 0.5, 1.0],
    [1.0, 0.5, 0.5, 1.0],
    [0.5, 1.0, 1.0, 1.0],
    [1.0, 0.5, 1.0, 1.0],
];

const SPEEDS: [f32; 6] = [
    0.7, -1.0, 1.3, -1.6, 1.9, -2.2
];

fn main() {
    // feed Froggy
    let node_store = froggy::Storage::new();
    let material_store = froggy::Storage::new();
    {
        let mut materials = material_store.write();
        for &color in COLORS.iter() {
            materials.create(Material {
                color: color,
            });
        }
    }
    let level_store = froggy::Storage::new();
    {
        let mut levels = level_store.write();
        for &speed in SPEEDS.iter() {
            levels.create(Level {
                speed: speed,
            });
        }
    }

    //Note: we populated the storages, but the returned pointers are already dropped.
    // Thus, all will be lost if we lock for writing now, but locking for reading retains the
    // contents, and cube creation will add references to them, so they will stay alive.
    let mut cubes = create_cubes(node_store.write(), material_store.read(), level_store.read());
    println!("Initialized {} cubes on {} levels", cubes.len(), SPEEDS.len());

    // init window and graphics
    let builder = glutin::WindowBuilder::new()
        .with_title("Froggy Cube-seption".to_string())
        .with_vsync();
    let (window, mut device, mut factory, main_color, main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(builder);
    let mut encoder = gfx::Encoder::from(factory.create_command_buffer());

    let (cube_vbuf, mut cube_slice) = create_geometry(&mut factory);
    let cube_ibuf = factory.create_buffer(cubes.len(),
        gfx::buffer::Role::Vertex, gfx::memory::Usage::Dynamic, gfx::Bind::empty()
        ).unwrap();

    // init global parameters
    let camera_pos = cgmath::Point3::new(-1.8, -8.0, 3.0);
    let globals = {
        let (w, h, _, _) = main_color.get_dimensions();
        make_globals(camera_pos, w as f32 / h as f32)
    };
    let global_buf = factory.create_constant_buffer(1);
    encoder.update_constant_buffer(&global_buf, &globals);

    // init pipeline states
    let pso = factory.create_pipeline_simple(
        include_bytes!("vert.glsl"),
        include_bytes!("frag.glsl"),
        pipe::new()
        ).unwrap();
    let mut data = pipe::Data {
        vert_buf: cube_vbuf,
        inst_buf: cube_ibuf,
        globals: global_buf,
        out_color: main_color,
        out_depth: main_depth,
    };

    let mut instances = Vec::new();
    let mut moment = time::Instant::now();

    'main: loop {
        // process events
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'main,
                glutin::Event::Resized(width, height) => {
                    gfx_window_glutin::update_views(&window, &mut data.out_color, &mut data.out_depth);
                    let globals = make_globals(camera_pos, width as f32 / height as f32);
                    encoder.update_constant_buffer(&data.globals, &globals);
                },
                _ => (),
            }
        }
        // get time delta
        let duration = moment.elapsed();
        let delta = duration.as_secs() as f32 + (duration.subsec_nanos() as f32 * 1.0e-9);
        moment = time::Instant::now();

        // Note: the following 3 passes could be combined into one for efficiency.
        // This is not the goal of the demo though. It is made with an assuption that
        // the scenegraph and its logic is separate from the main game, and that
        // more processing is done in parallel.

        // animate local spaces
        {
            let mut nodes = node_store.write();
            let levels = level_store.read();
            for cube in cubes.iter_mut() {
                let node = nodes.access(&cube.node);
                let level = levels.access(&cube.level);
                let angle = cgmath::Rad(delta * level.speed);
                node.local.concat_self(&Space {
                    disp: cgmath::Vector3::zero(),
                    rot: cgmath::Quaternion::from_angle_z(angle),
                    scale: 1.0,
                });
            }
        }

        // re-compute world spaces
        {
            let mut nodes = node_store.write();
            let mut dummy = Node {
                local: Space::one(),
                world: Space::one(),
                parent: None,
            };
            for i in 0 .. nodes.len() {
                // replacing with dummy instead of cloning here - to avoid the refcount bumps
                let mut node = mem::replace(&mut nodes[i], dummy);
                node.world = match node.parent {
                    Some(ref parent) => nodes.access(parent).world.concat(&node.local),
                    None => node.local,
                };
                dummy = mem::replace(&mut nodes[i], node);
            }
        }

        // update instancing CPU info
        instances.clear();
        {
            let mut nodes = node_store.write();
            let materials = material_store.read();
            for cube in cubes.iter_mut() {
                let material = materials.access(&cube.material);
                let space = &nodes.access(&cube.node).world;
                instances.push(Instance {
                    offset_scale: space.disp.extend(space.scale).into(),
                    rotation: space.rot.v.extend(space.rot.s).into(),
                    color: material.color,
                });
            }
        }

        // update instancing GPU info
        cube_slice.instances = Some((instances.len() as gfx::InstanceCount, 0));
        encoder.update_buffer(&data.inst_buf, &instances, 0).unwrap();

        // draw -- start
        encoder.clear_depth(&data.out_depth, 1.0);
        encoder.clear(&data.out_color, [0.1, 0.2, 0.3, 1.0]);
        encoder.draw(&cube_slice, &pso, &data);
        // draw -- end
        encoder.flush(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
