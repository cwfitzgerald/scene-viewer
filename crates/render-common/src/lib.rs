use glam::UVec2;

pub trait Renderer {
    fn resize(&mut self, resolution: UVec2) -> anyhow::Result<()>;

    fn render(&mut self) -> anyhow::Result<()>;
}
