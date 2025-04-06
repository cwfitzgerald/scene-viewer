use anyhow::Context;
use ash::vk;

use crate::DeviceShared;

pub struct MeshRenderer {
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl MeshRenderer {
    pub fn new(shared: &DeviceShared) -> anyhow::Result<Self> {
        unsafe {
            let pipeline_layout = shared
                .device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&[])
                        .push_constant_ranges(&[]),
                    None,
                )
                .context("Failed to create pipeline layout")?;

            let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
                .layout(pipeline_layout)
                .render_pass(vk::RenderPass::null());
            let pipeline = shared
                .device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_create_info], None)
                .map_err(|(_, e)| e)
                .context("Failed to create mesh renderer pipeline")?[0];

            Ok(Self {
                pipeline_layout,
                pipeline,
            })
        }
    }
}
