extern crate cgmath;
extern crate froggy;
#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;

use gfx::format::{I8Norm, U8Norm};


pub type ColorFormat = gfx::format::Srgba8;
pub type DepthFormat = gfx::format::DepthStencil;

gfx_defines!{
    vertex Vertex {
        pos: [f32; 4] = "a_Pos",
        normal: [I8Norm; 4] = "a_Normal",
    }

    vertex Instance {
        offset_scale: [f32; 4] = "a_OffsetScale",
        rotation: [f32; 4] = "a_Rotation",
        color: [U8Norm; 4] = "a_Color",
    }

    constant Globals {
        light_pos: [f32; 4] = "u_LightPos",
        light_color: [f32; 4] = "u_LightColor",
    }

    pipeline pipe {
        vert_buf: gfx::VertexBuffer<Vertex> = (),
        inst_buf: gfx::InstanceBuffer<Instance> = (),
        out_color: gfx::RenderTarget<ColorFormat> = "Target0",
        out_depth: gfx::DepthTarget<DepthFormat> = gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

fn vertex(x: i8, y: i8, z: i8, nx: i8, ny: i8, nz: i8) -> Vertex {
    Vertex {
        pos: [x as f32, y as f32, z as f32, 1.0],
        normal: I8Norm::cast4([nx, ny, nz, 0]),
    }
}

const CODE_VS: &'static [u8] = b"
    #version 150 core
    in vec4 a_Pos;

    vec3 rotate_vector(vec4 quat, vec3 vec)
    {
        return vec + 2.0 * cross(cross(vec, quat.xyz) + quat.w * vec, quat.xyz);
    }

    void main() {
        gl_Position = a_Pos;
    }
";

const CODE_FS: &'static [u8] = b"
    #version 150 core
    void main() {
        gl_FragColor = vec4(1.0, 1.0, 1.0, 1.0);
    }
";


fn main() {
    use gfx::traits::{Device, FactoryExt};

    let builder = glutin::WindowBuilder::new()
        .with_title("Froggy Cubes demo".to_string())
        .with_vsync();
    let (window, mut device, mut factory, main_color, _main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(builder);

    let mut encoder = gfx::Encoder::from(factory.create_command_buffer());
    let pso = factory.create_pipeline_simple(CODE_VS, CODE_FS, pipe::new()).unwrap();

    'main: loop {
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'main,
                _ => (),
            }
        }
        // draw -- start
        encoder.clear(&main_color, [0.1, 0.2, 0.3, 1.0]);
        // draw -- end
        encoder.flush(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
