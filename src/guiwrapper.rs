#[allow(non_snake_case)]

use std::process::Command;
use glium::Surface;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, NotCurrentGlContext},
    display::{GetGlDisplay, GlDisplay},
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use imgui_winit_support::winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};
use raw_window_handle::HasRawWindowHandle;
use winit::{
    event::{Event, WindowEvent},
    window::Window,
};

fn main()
{
    // Command::new("target/debug/simulator")
    //     .spawn()
    //     .expect("Failed to execute simulator.");
    // let (_, _, _) = create_window();
}

// fn create_window() -> (EventLoop<()>, Window, glium::Display<WindowSurface>) {
//     let event_loop = EventLoop::new().expect("Failed to create EventLoop");

//     let window_builder = WindowBuilder::new()
//         .with_title(TITLE)
//         .with_inner_size(LogicalSize::new(1024, 768));

//     let (window, cfg) = glutin_winit::DisplayBuilder::new()
//         .with_window_builder(Some(window_builder))
//         .build(&event_loop, ConfigTemplateBuilder::new(), |mut configs| {
//             configs.next().unwrap()
//         })
//         .expect("Failed to create OpenGL window");
//     let window = window.unwrap();

//     let context_attribs = ContextAttributesBuilder::new().build(Some(window.raw_window_handle()));
//     let context = unsafe {
//         cfg.display()
//             .create_context(&cfg, &context_attribs)
//             .expect("Failed to create OpenGL context")
//     };

//     let surface_attribs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
//         window.raw_window_handle(),
//         NonZeroU32::new(1024).unwrap(),
//         NonZeroU32::new(768).unwrap(),
//     );
//     let surface = unsafe {
//         cfg.display()
//             .create_window_surface(&cfg, &surface_attribs)
//             .expect("Failed to create OpenGL surface")
//     };

//     let context = context
//         .make_current(&surface)
//         .expect("Failed to make OpenGL context current");

//     let display = glium::Display::from_context_surface(context, surface)
//         .expect("Failed to create glium Display");

//     (event_loop, window, display)
// }
