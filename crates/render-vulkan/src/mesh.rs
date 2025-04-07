use anyhow::Context;
use ash::vk;

use crate::DeviceShared;

pub struct MeshRenderer {
    pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
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

            let vertex: &[u32] =
                bytemuck::cast_slice(include_bytes!("../shaders/triangle.vert.spv"));
            let fragment: &[u32] =
                bytemuck::cast_slice(include_bytes!("../shaders/triangle.frag.spv"));

            let vertex_shader_module = shared
                .device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(vertex), None)
                .context("Failed to create vertex shader module")?;

            let fragment_shader_module = shared
                .device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(fragment), None)
                .context("Failed to create fragment shader module")?;

            let vertex_shader_stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex_shader_module)
                .name(c"main");

            let fragment_shader_stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fragment_shader_module)
                .name(c"main");

            let shader_stages = [vertex_shader_stage, fragment_shader_stage];

            let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default();
            let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let tessellation_state = vk::PipelineTessellationStateCreateInfo::default();
            let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1)
                .scissor_count(1);
            let pipeline_rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::NONE)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .line_width(1.0);
            let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default();

            let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(false)
                .color_write_mask(vk::ColorComponentFlags::RGBA);

            let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
                .logic_op_enable(false)
                .attachments(std::slice::from_ref(&color_blend_attachment));

            let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

            let dynamic_state =
                vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

            let mut dynamic_rendering = vk::PipelineRenderingCreateInfo::default()
                .color_attachment_formats(&[vk::Format::R8G8B8A8_SRGB]);

            let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
                .layout(pipeline_layout)
                .stages(&shader_stages)
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly_state)
                .tessellation_state(&tessellation_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&pipeline_rasterization_state)
                .multisample_state(&multisample_state)
                .depth_stencil_state(&depth_stencil_state)
                .color_blend_state(&color_blend_state)
                .dynamic_state(&dynamic_state)
                .render_pass(vk::RenderPass::null())
                .push_next(&mut dynamic_rendering);

            let pipeline = shared
                .device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_create_info], None)
                .map_err(|(_, e)| e)
                .context("Failed to create mesh renderer pipeline")?[0];

            shared
                .device
                .destroy_shader_module(vertex_shader_module, None);
            shared
                .device
                .destroy_shader_module(fragment_shader_module, None);

            Ok(Self {
                pipeline_layout,
                pipeline,
            })
        }
    }

    pub fn destroy(&self, shared: &DeviceShared) {
        unsafe {
            shared.device.destroy_pipeline(self.pipeline, None);
            shared
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}
