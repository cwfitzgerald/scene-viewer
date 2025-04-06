use glam::UVec2;

pub trait Renderer {
    fn resize(&mut self, resolution: UVec2) -> anyhow::Result<()>;
}
