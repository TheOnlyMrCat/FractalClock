use pollster::FutureExt;
use wgpu_fractal_clock::Renderer;

use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::WindowBuilder;

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Fractal Clock")
        .build(&event_loop)
        .unwrap();

    let mut renderer = Renderer::new(10, &window, window.inner_size().into()).block_on();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,
        Event::WindowEvent { event: WindowEvent::Resized(new_size), .. } => {
            renderer.resize(new_size.into());
        }
        Event::MainEventsCleared => {
            renderer.render();
        },
        _ => {}
    });
}
