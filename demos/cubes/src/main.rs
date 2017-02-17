extern crate cgmath;
extern crate froggy;
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

use cgmath::{EuclideanSpace, Transform};
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

const CODE_VS: &'static [u8] = b"
    #version 150 core
    in vec4 a_Pos;

    uniform b_Globals {
        mat4 u_Projection;
        vec4 u_LightPos;
        vec4 u_LightColor;
    };

    vec3 rotate_vector(vec4 quat, vec3 vec) {
        return vec + 2.0 * cross(cross(vec, quat.xyz) + quat.w * vec, quat.xyz);
    }

    void main() {
        gl_Position = u_Projection * a_Pos;
    }
";

const CODE_FS: &'static [u8] = b"
    #version 150 core
    void main() {
        gl_FragColor = vec4(1.0, 1.0, 1.0, 1.0);
    }
";


fn create_cube<R: gfx::Resources, F: gfx::Factory<R>>(factory: &mut F)
               -> (gfx::handle::Buffer<R, Vertex>, gfx::Slice<R>)
{
    let vertices = [
        // bottom
        vertex(1, 1, -1, 0, 0, -1),
        vertex(1, -1, -1, 0, 0, -1),
        vertex(-1, -1, -1, 0, 0, -1),
        vertex(-1, 1, -1, 0, 0, -1),
        // top
        vertex(1, 1, 1, 0, 0, 1),
        vertex(1, -1, 1, 0, 0, 1),
        vertex(-1, -1, 1, 0, 0, 1),
        vertex(-1, 1, 1, 0, 0, 1),
        // left
        vertex(-1, 1, 1, -1, 0, 0),
        vertex(-1, 1, -1, -1, 0, 0),
        vertex(-1, -1, -1, -1, 0, 0),
        vertex(-1, -1, 1, -1, 0, 0),
        // right
        vertex(1, 1, 1, 1, 0, 0),
        vertex(1, 1, -1, 1, 0, 0),
        vertex(1, -1, -1, 1, 0, 0),
        vertex(1, -1, 1, 1, 0, 0),
        // front
        vertex(1, -1, 1, 0, -1, 0),
        vertex(1, -1, -1, 0, -1, 0),
        vertex(-1, -1, -1, 0, -1, 0),
        vertex(-1, -1, 1, 0, -1, 0),
        // back
        vertex(1, 1, 1, 0, 1, 0),
        vertex(1, 1, -1, 0, 1, 0),
        vertex(-1, 1, -1, 0, 1, 0),
        vertex(-1, 1, 1, 0, 1, 0),
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


fn main() {
    let builder = glutin::WindowBuilder::new()
        .with_title("Froggy Cubes demo".to_string())
        .with_vsync();
    let (window, mut device, mut factory, main_color, main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(builder);
    let mut encoder = gfx::Encoder::from(factory.create_command_buffer());

    let (cube_vbuf, cube_slice) = create_cube(&mut factory);
    let instances = [
        Instance {
            offset_scale: [0.0, 0.0, 0.0, 1.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
    ];
    let cube_ibuf = factory.create_buffer(1, gfx::buffer::Role::Vertex,
        gfx::memory::Usage::Dynamic, gfx::Bind::empty()).unwrap();
    encoder.update_buffer(&cube_ibuf, &instances, 0).unwrap();

    let mx_proj = {
        let (w, h, _, _) = main_color.get_dimensions();
        let fovy = cgmath::Deg(60.0);
        let aspect = w as f32 / h as f32;
        let perspective = cgmath::perspective(fovy, aspect, 1.0, 100.0);
        let view: cgmath::Matrix4<f32> = Transform::look_at(
            cgmath::Point3::new(-2.0, -10.0, 4.0),
            cgmath::Point3::origin(),
            cgmath::Vector3::unit_z());
        perspective * view
    };
    let global_buf = factory.create_constant_buffer(1);
    encoder.update_constant_buffer(&global_buf, &Globals {
        projection: mx_proj.into(),
        light_pos: [10.0, 10.0, 10.0, 1.0],
        light_color: [1.0, 1.0, 0.0, 1.0],
    });

    let pso = factory.create_pipeline_simple(CODE_VS, CODE_FS, pipe::new()).unwrap();
    let data = pipe::Data {
        vert_buf: cube_vbuf,
        inst_buf: cube_ibuf,
        globals: global_buf,
        out_color: main_color,
        out_depth: main_depth,
    };

    'main: loop {
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'main,
                _ => (),
            }
        }
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
