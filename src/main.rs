use pollster::FutureExt;
use wgpu_fractal_clock::Renderer;

use winit::event::{Event, WindowEvent, VirtualKeyCode, ElementState};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::WindowBuilder;

fn main() {
    let event_loop = EventLoop::new();

    #[cfg(target_os = "macos")]
    unsafe {
        // work-around for https://github.com/rust-windowing/winit/issues/2051
        use cocoa::appkit::NSApplication as _;
        cocoa::appkit::NSApp().setActivationPolicy_(
            cocoa::appkit::NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
        );
    }

    let window = WindowBuilder::new()
        .with_title("Fractal Clock")
        .build(&event_loop)
        .unwrap();

    let mut depth = 4;
    let mut renderer = Renderer::new(depth, &window, window.inner_size().into()).block_on();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,
        Event::WindowEvent { event: WindowEvent::Resized(new_size), .. } => {
            renderer.resize(new_size.into());
        }
        Event::WindowEvent { event: WindowEvent::KeyboardInput { input, .. }, .. } => {
            if input.state == ElementState::Pressed {
                match input.virtual_keycode {
                    Some(VirtualKeyCode::Up) => depth += 1,
                    Some(VirtualKeyCode::Down) if depth > 1 => depth -= 1,
                    _ => (),
                }
                renderer.set_depth(depth);
            }
        }
        Event::MainEventsCleared => {
            renderer.render();
        },
        _ => {}
    });
}
