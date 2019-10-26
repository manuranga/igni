extern crate shaderc;
extern crate chrono;
extern crate time;
use git2::{Commit, Repository, Time};
use git2::{Error};
use std::str;
use wgpu_glyph::{GlyphBrushBuilder, Section};
use chrono::{Utc};

fn main() {

    match run() {
        Ok(()) => {}
        Err(e) => println!("error: {}", e)
    }

    use winit::{
        event_loop::{ControlFlow, EventLoop},
        event,
    };

    env_logger::init();
    let event_loop = EventLoop::new();

    #[cfg(not(feature = "gl"))]
    let (_window, size, surface) = {
        let window = winit::window::Window::new(&event_loop).unwrap();
        let size = window
            .inner_size()
            .to_physical(window.hidpi_factor());

        let surface = wgpu::Surface::create(&window);
        (window, size, surface)
    };

    #[cfg(feature = "gl")]
    let (_window, instance, size, surface) = {
        let wb = winit::WindowBuilder::new();
        let cb = wgpu::glutin::ContextBuilder::new().with_vsync(true);
        let context = cb.build_windowed(wb, &event_loop).unwrap();

        let size = context
            .window()
            .get_inner_size()
            .unwrap()
            .to_physical(context.window().get_hidpi_factor());

        let (context, window) = unsafe { context.make_current().unwrap().split() };

        let instance = wgpu::Instance::new(context);
        let surface = instance.get_surface();

        (window, instance, size, surface)
    };

    let adapter = wgpu::Adapter::request(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::Default,
        backends: wgpu::BackendBit::PRIMARY,
    }).unwrap();

    let (mut device, mut queue) = adapter.request_device(&wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default(),
    });

    let vs_bytes = load_glsl(include_str!("shader.vert"), shaderc::ShaderKind::Vertex);
    let vs_module = device.create_shader_module(&vs_bytes);

    let fs_bytes = load_glsl(include_str!("shader.frag"), shaderc::ShaderKind::Fragment);
    let fs_module = device.create_shader_module(&fs_bytes);

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        bindings: &[],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vs_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &fs_module,
            entry_point: "main",
        }),
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        color_states: &[wgpu::ColorStateDescriptor {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_blend: wgpu::BlendDescriptor::REPLACE,
            alpha_blend: wgpu::BlendDescriptor::REPLACE,
            write_mask: wgpu::ColorWrite::ALL,
        }],
        depth_stencil_state: None,
        index_format: wgpu::IndexFormat::Uint16,
        vertex_buffers: &[],
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    });

    let render_format = wgpu::TextureFormat::Bgra8UnormSrgb;

    let mut swap_chain = device.create_swap_chain(
        &surface,
        &wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: render_format,
            width: size.width.round() as u32,
            height: size.height.round() as u32,
            present_mode: wgpu::PresentMode::Vsync,
        },
    );

    // Prepare glyph_brush
    let inconsolata: &[u8] = include_bytes!("../res/Inconsolata-Regular.ttf");
    let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(inconsolata)
        .build(&mut device, render_format);

    let mut frames = 0.0;
    let mut time = Utc::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = if cfg!(feature = "metal-auto-capture") {
            ControlFlow::Exit
        } else {
            ControlFlow::Poll
        };
        match event {
            event::Event::WindowEvent { event, .. } => match event {
                event::WindowEvent::KeyboardInput {
                    input:
                        event::KeyboardInput {
                            virtual_keycode: Some(event::VirtualKeyCode::Escape),
                            state: event::ElementState::Pressed,
                            ..
                        },
                    ..
                }
                | event::WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            event::Event::EventsCleared => {
                let frame = swap_chain.get_next_texture();
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &frame.view,
                            resolve_target: None,
                            load_op: wgpu::LoadOp::Clear,
                            store_op: wgpu::StoreOp::Store,
                            clear_color: wgpu::Color::GREEN,
                        }],
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&render_pipeline);
                    rpass.set_bind_group(0, &bind_group, &[]);
                    rpass.draw(0 .. 3, 0 .. 1);
                }

                frames += 1.0;
                let time_now = Utc::now();
                let duration = (time_now - time).num_milliseconds() as f64;
                glyph_brush.queue(Section {
                    text: &(format!("{:.*}", 2, frames/duration*1000.0) + " fps"),
                    ..Section::default()
                });
                if duration > 1000.0 {
                    time = time_now;
                    frames = 0.0;
                }

                // Draw the text!
                glyph_brush
                    .draw_queued(
                        &mut device,
                        &mut encoder,
                        &frame.view,
                        size.width.round() as u32,
                        size.height.round() as u32,
                    )
                    .expect("Draw queued");

                queue.submit(&[encoder.finish()]);
            }
            _ => (),
        }
    });
}

pub fn load_glsl(code: &str, stage:  shaderc::ShaderKind) -> Vec<u32> {
    let mut compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();
    let binary_result = compiler.compile_into_spirv(
        code, stage,
        "shader.glsl", "main", Some(&options)).unwrap();
    let spirv_bin = binary_result.as_binary_u8();
    wgpu::read_spirv(std::io::Cursor::new(&spirv_bin[..])).unwrap()
}

fn run() -> Result<(), Error> {
    let repo = Repository::open(".")?;
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL);
    revwalk.push_head()?;

    let revwalk = revwalk
        .filter_map(|id| {
            let id = id.unwrap();
            let commit = repo.find_commit(id).unwrap();
            Some(commit)
        })
        .skip(0)
        .take(std::usize::MAX);

    // print!
    for commit in revwalk {
        print_commit(&commit);
    }

    Ok(())
}

fn print_commit(commit: &Commit) {
    println!("commit {}", commit.id());

    if commit.parents().len() > 1 {
        print!("Merge:");
        for id in commit.parent_ids() {
            print!(" {:.8}", id);
        }
        println!();
    }

    let author = commit.author();
    println!("Author: {}", author);
    print_time(&author.when(), "Date:   ");
    println!();

    for line in String::from_utf8_lossy(commit.message_bytes()).lines() {
        println!("    {}", line);
    }
    println!();
}

fn print_time(time: &Time, prefix: &str) {
    let (offset, sign) = match time.offset_minutes() {
        n if n < 0 => (-n, '-'),
        n => (n, '+'),
    };
    let (hours, minutes) = (offset / 60, offset % 60);
    let ts = time::Timespec::new(time.seconds() + (time.offset_minutes() as i64) * 60, 0);
    let time = time::at(ts);

    println!(
        "{}{} {}{:02}{:02}",
        prefix,
        time.strftime("%a %b %e %T %Y").unwrap(),
        sign,
        hours,
        minutes
    );
}