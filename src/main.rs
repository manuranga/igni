extern crate chrono;
extern crate shaderc;
extern crate time;
use chrono::Utc;
use git2::Error;
use git2::{Commit, Repository};
use std::str;
use wgpu_glyph::{GlyphBrushBuilder, Scale, Section};
pub mod model;
use model::{GApp, GCommit};
use std::sync::{Arc, RwLock};
use std::thread;

fn main() {
    use winit::{
        event,
        event_loop::{ControlFlow, EventLoop},
    };

    env_logger::init();
    let event_loop = EventLoop::new();

    #[cfg(not(feature = "gl"))]
    let (_window, size, surface) = {
        let window = winit::window::Window::new(&event_loop).unwrap();
        let size = window.inner_size().to_physical(window.hidpi_factor());

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
    })
    .unwrap();

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

    let bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { bindings: &[] });
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
    let mut glyph_brush =
        GlyphBrushBuilder::using_font_bytes(inconsolata).build(&mut device, render_format);

    let state: Arc<RwLock<Option<GApp>>> = Arc::new(RwLock::new(None));
    let state_reader = state.clone();

    thread::spawn(move || {
        let repo = Repository::open(".").unwrap();
        let commits = list_commits(&repo).unwrap();
        let commits: Vec<GCommit> = commits
            .iter()
            .map(|c| GCommit {
                author: String::from(c.author().name().unwrap()),
                id: c.id().to_string(),
            })
            .collect();
        let mut shared_app = state.write().unwrap();
        let app: GApp = GApp { commits };
        *shared_app = Some(app);
    });

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
                    rpass.draw(0..3, 0..1);
                }

                let ptr_opt_app: &Option<GApp> = &state_reader.read().unwrap();
                let opt_app: Option<&GApp> = ptr_opt_app.as_ref();

                match opt_app {
                    Some(app) => {
                        let mut h = 0.0;
                        let scale = Scale { x: 40.0, y: 40.0 };
                        for commit in &app.commits {
                            h += 30.0;
                            glyph_brush.queue(Section {
                                text: &commit.author,
                                screen_position: (30.0, h),
                                scale,
                                ..Section::default()
                            });

                            glyph_brush.queue(Section {
                                text: &commit.id,
                                screen_position: (500.0, h),
                                scale,
                                ..Section::default()
                            });
                        }
                    }
                    None => {}
                }

                frames += 1.0;
                let time_now = Utc::now();
                let duration = (time_now - time).num_milliseconds() as f64;
                glyph_brush.queue(Section {
                    text: &(format!("{:.*}", 2, frames / duration * 1000.0) + " fps"),
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

pub fn load_glsl(code: &str, stage: shaderc::ShaderKind) -> Vec<u32> {
    let mut compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();
    let binary_result = compiler
        .compile_into_spirv(code, stage, "shader.glsl", "main", Some(&options))
        .unwrap();
    let spirv_bin = binary_result.as_binary_u8();
    wgpu::read_spirv(std::io::Cursor::new(&spirv_bin[..])).unwrap()
}

fn list_commits<'a>(repo: &'a Repository) -> Result<Vec<Commit<'a>>, Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL);
    revwalk.push_head()?;

    let revwalk: Vec<_> = revwalk
        .filter_map(|id| {
            let id = id.unwrap();
            let commit = repo.find_commit(id).unwrap();
            Some(commit)
        })
        .collect();

    Ok(revwalk)
}
