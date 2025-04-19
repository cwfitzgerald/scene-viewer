use anyhow::Context;
use glam::UVec2;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use render_common::Renderer;
use sdl3::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
};

fn main() -> anyhow::Result<()> {
    let sdl_ctx = sdl3::init().context("Failed to initialize SDL3")?;
    let video_subsystem = sdl_ctx.video().context("Failed to get video subsystem")?;

    let window = video_subsystem
        .window("Scene Viewer", 2560, 1440)
        .vulkan()
        .high_pixel_density()
        .resizable()
        .build()
        .context("Failed to create SDL3 window")?;

    let display_handle = window.display_handle().context("Failed to get display handle")?;
    let window_handle = window.window_handle().context("Failed to get window handle")?;

    let size = UVec2::from(window.size_in_pixels());

    let mut renderer =
        render_vulkan::VulkanRenderer::new(display_handle.as_raw(), window_handle.as_raw(), size)
            .context("Failed to create Vulkan renderer")?;

    let mut event_pump = sdl_ctx.event_pump().context("Failed to create event pump")?;

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                }
                Event::Window { win_event: WindowEvent::Resized(x, y), .. } => {
                    renderer.resize(UVec2::new(x as u32, y as u32)).unwrap()
                }
                _ => {}
            }
        }

        renderer.render().context("Failed to render")?;
    }

    renderer.dispose();

    Ok(())
}
