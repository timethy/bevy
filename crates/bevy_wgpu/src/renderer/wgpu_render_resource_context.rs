use crate::{
    wgpu_type_converter::{OwnedWgpuVertexBufferDescriptor, WgpuInto},
    WgpuBindGroupInfo, WgpuResources,
};

use bevy_asset::{AssetStorage, Handle, HandleUntyped};
use bevy_render::{
    pipeline::{BindGroupDescriptor, PipelineDescriptor},
    render_resource::{
        BufferInfo, RenderResource, RenderResourceAssignment, RenderResourceAssignments,
        RenderResourceSetId, ResourceInfo,
    },
    renderer::RenderResourceContext,
    shader::Shader,
    texture::{Extent3d, SamplerDescriptor, TextureDescriptor},
};
use bevy_window::{Window, WindowId};
use std::sync::Arc;

#[derive(Clone)]
pub struct WgpuRenderResourceContext {
    pub device: Arc<wgpu::Device>,
    pub resources: WgpuResources,
}

impl WgpuRenderResourceContext {
    pub fn new(device: Arc<wgpu::Device>) -> Self {
        WgpuRenderResourceContext {
            device,
            resources: WgpuResources::default(),
        }
    }

    pub fn set_window_surface(&self, window_id: WindowId, surface: wgpu::Surface) {
        let mut window_surfaces = self.resources.window_surfaces.write().unwrap();
        window_surfaces.insert(window_id, surface);
    }

    pub fn create_texture_with_data(
        &mut self,
        command_encoder: &mut wgpu::CommandEncoder,
        texture_descriptor: TextureDescriptor,
        bytes: &[u8],
    ) -> RenderResource {
        let mut resource_info = self.resources.resource_info.write().unwrap();
        let mut texture_views = self.resources.texture_views.write().unwrap();
        let mut textures = self.resources.textures.write().unwrap();

        let descriptor: wgpu::TextureDescriptor = (&texture_descriptor).wgpu_into();
        let texture = self.device.create_texture(&descriptor);
        let texture_view = texture.create_default_view();
        let temp_buf = self
            .device
            .create_buffer_with_data(bytes, wgpu::BufferUsage::COPY_SRC);
        command_encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: &temp_buf,
                offset: 0,
                bytes_per_row: 4 * descriptor.size.width,
                rows_per_image: 0, // NOTE: Example sets this to 0, but should it be height?
            },
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                array_layer: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
            },
            descriptor.size,
        );

        let resource = RenderResource::new();
        resource_info.insert(resource, ResourceInfo::Texture(texture_descriptor));
        texture_views.insert(resource, texture_view);
        textures.insert(resource, texture);

        resource
    }

    pub fn copy_buffer_to_buffer(
        &self,
        command_encoder: &mut wgpu::CommandEncoder,
        source_buffer: RenderResource,
        source_offset: u64,
        destination_buffer: RenderResource,
        destination_offset: u64,
        size: u64,
    ) {
        let buffers = self.resources.buffers.read().unwrap();

        let source = buffers.get(&source_buffer).unwrap();
        let destination = buffers.get(&destination_buffer).unwrap();
        command_encoder.copy_buffer_to_buffer(
            source,
            source_offset,
            destination,
            destination_offset,
            size,
        );
    }

    pub fn copy_buffer_to_texture(
        &self,
        command_encoder: &mut wgpu::CommandEncoder,
        source_buffer: RenderResource,
        source_offset: u64,
        source_bytes_per_row: u32,
        destination_texture: RenderResource,
        destination_origin: [u32; 3], // TODO: replace with math type
        destination_mip_level: u32,
        destination_array_layer: u32,
        size: Extent3d,
    ) {
        let buffers = self.resources.buffers.read().unwrap();
        let textures = self.resources.textures.read().unwrap();

        let source = buffers.get(&source_buffer).unwrap();
        let destination = textures.get(&destination_texture).unwrap();
        command_encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: source,
                offset: source_offset,
                bytes_per_row: source_bytes_per_row,
                rows_per_image: 0, // NOTE: Example sets this to 0, but should it be height?
            },
            wgpu::TextureCopyView {
                texture: destination,
                mip_level: destination_mip_level,
                array_layer: destination_array_layer,
                origin: wgpu::Origin3d {
                    x: destination_origin[0],
                    y: destination_origin[1],
                    z: destination_origin[2],
                },
            },
            size.wgpu_into(),
        );
    }

    pub fn create_bind_group_layout(&self, descriptor: &BindGroupDescriptor) {
        if self
            .resources
            .bind_group_layouts
            .read()
            .unwrap()
            .get(&descriptor.id)
            .is_some()
        {
            return;
        }

        let mut bind_group_layouts = self.resources.bind_group_layouts.write().unwrap();
        // TODO: consider re-checking existence here
        let bind_group_layout_binding = descriptor
            .bindings
            .iter()
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding: binding.index,
                visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                ty: (&binding.bind_type).wgpu_into(),
            })
            .collect::<Vec<wgpu::BindGroupLayoutEntry>>();
        let wgpu_descriptor = wgpu::BindGroupLayoutDescriptor {
            bindings: bind_group_layout_binding.as_slice(),
            label: None,
        };
        let bind_group_layout = self.device.create_bind_group_layout(&wgpu_descriptor);
        bind_group_layouts.insert(descriptor.id, bind_group_layout);
    }
}

impl RenderResourceContext for WgpuRenderResourceContext {
    fn create_sampler(&self, sampler_descriptor: &SamplerDescriptor) -> RenderResource {
        let mut samplers = self.resources.samplers.write().unwrap();
        let mut resource_info = self.resources.resource_info.write().unwrap();

        let descriptor: wgpu::SamplerDescriptor = (*sampler_descriptor).wgpu_into();
        let sampler = self.device.create_sampler(&descriptor);

        let resource = RenderResource::new();
        samplers.insert(resource, sampler);
        resource_info.insert(resource, ResourceInfo::Sampler);
        resource
    }

    fn create_texture(&self, texture_descriptor: TextureDescriptor) -> RenderResource {
        let mut textures = self.resources.textures.write().unwrap();
        let mut texture_views = self.resources.texture_views.write().unwrap();
        let mut resource_info = self.resources.resource_info.write().unwrap();

        let descriptor: wgpu::TextureDescriptor = (&texture_descriptor).wgpu_into();
        let texture = self.device.create_texture(&descriptor);
        let texture_view = texture.create_default_view();

        let resource = RenderResource::new();
        resource_info.insert(resource, ResourceInfo::Texture(texture_descriptor));
        texture_views.insert(resource, texture_view);
        textures.insert(resource, texture);
        resource
    }

    fn create_buffer(&self, buffer_info: BufferInfo) -> RenderResource {
        // TODO: consider moving this below "create" for efficiency
        let mut resource_info = self.resources.resource_info.write().unwrap();
        let mut buffers = self.resources.buffers.write().unwrap();

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: buffer_info.size as u64,
            usage: buffer_info.buffer_usage.wgpu_into(),
        });

        let resource = RenderResource::new();
        resource_info.insert(resource, ResourceInfo::Buffer(buffer_info));
        buffers.insert(resource, buffer);
        resource
    }

    fn create_buffer_mapped(
        &self,
        buffer_info: BufferInfo,
        setup_data: &mut dyn FnMut(&mut [u8], &dyn RenderResourceContext),
    ) -> RenderResource {
        let mut mapped = self.device.create_buffer_mapped(&wgpu::BufferDescriptor {
            size: buffer_info.size as u64,
            usage: buffer_info.buffer_usage.wgpu_into(),
            label: None,
        });
        setup_data(&mut mapped.data(), self);
        let buffer = mapped.finish();

        let resource = RenderResource::new();
        let mut resource_info = self.resources.resource_info.write().unwrap();
        let mut buffers = self.resources.buffers.write().unwrap();
        resource_info.insert(resource, ResourceInfo::Buffer(buffer_info));
        buffers.insert(resource, buffer);
        resource
    }

    fn create_buffer_with_data(&self, mut buffer_info: BufferInfo, data: &[u8]) -> RenderResource {
        // TODO: consider moving this below "create" for efficiency
        let mut resource_info = self.resources.resource_info.write().unwrap();
        let mut buffers = self.resources.buffers.write().unwrap();

        buffer_info.size = data.len();
        let buffer = self
            .device
            .create_buffer_with_data(data, buffer_info.buffer_usage.wgpu_into());

        let resource = RenderResource::new();
        resource_info.insert(resource, ResourceInfo::Buffer(buffer_info));
        buffers.insert(resource, buffer);
        resource
    }

    fn remove_buffer(&self, resource: RenderResource) {
        let mut buffers = self.resources.buffers.write().unwrap();
        let mut resource_info = self.resources.resource_info.write().unwrap();

        buffers.remove(&resource);
        resource_info.remove(&resource);
    }

    fn remove_texture(&self, resource: RenderResource) {
        let mut textures = self.resources.textures.write().unwrap();
        let mut texture_views = self.resources.texture_views.write().unwrap();
        let mut resource_info = self.resources.resource_info.write().unwrap();

        textures.remove(&resource);
        texture_views.remove(&resource);
        resource_info.remove(&resource);
    }

    fn remove_sampler(&self, resource: RenderResource) {
        let mut samplers = self.resources.samplers.write().unwrap();
        let mut resource_info = self.resources.resource_info.write().unwrap();

        samplers.remove(&resource);
        resource_info.remove(&resource);
    }

    fn get_resource_info(
        &self,
        resource: RenderResource,
        handle_info: &mut dyn FnMut(Option<&ResourceInfo>),
    ) {
        let resource_info = self.resources.resource_info.read().unwrap();
        let info = resource_info.get(&resource);
        handle_info(info);
    }

    fn create_shader_module_from_source(&self, shader_handle: Handle<Shader>, shader: &Shader) {
        let mut shader_modules = self.resources.shader_modules.write().unwrap();
        let shader_module = self.device.create_shader_module(&shader.get_spirv(None));
        shader_modules.insert(shader_handle, shader_module);
    }

    fn create_shader_module(
        &self,
        shader_handle: Handle<Shader>,
        shader_storage: &AssetStorage<Shader>,
    ) {
        if self
            .resources
            .shader_modules
            .read()
            .unwrap()
            .get(&shader_handle)
            .is_some()
        {
            return;
        }
        let shader = shader_storage.get(&shader_handle).unwrap();
        self.create_shader_module_from_source(shader_handle, shader);
    }

    fn create_swap_chain(&self, window: &Window) {
        let surfaces = self.resources.window_surfaces.read().unwrap();
        let mut window_swap_chains = self.resources.window_swap_chains.write().unwrap();

        let swap_chain_descriptor: wgpu::SwapChainDescriptor = window.wgpu_into();
        let surface = surfaces
            .get(&window.id)
            .expect("No surface found for window");
        let swap_chain = self
            .device
            .create_swap_chain(surface, &swap_chain_descriptor);

        window_swap_chains.insert(window.id, swap_chain);
    }

    fn next_swap_chain_texture(&self, window_id: bevy_window::WindowId) -> RenderResource {
        let mut window_swap_chains = self.resources.window_swap_chains.write().unwrap();
        let mut swap_chain_outputs = self.resources.swap_chain_outputs.write().unwrap();

        let window_swap_chain = window_swap_chains.get_mut(&window_id).unwrap();
        let next_texture = window_swap_chain.get_next_texture().unwrap();

        // TODO: Add ResourceInfo
        let render_resource = RenderResource::new();
        swap_chain_outputs.insert(render_resource, next_texture);
        render_resource
    }

    fn drop_swap_chain_texture(&self, render_resource: RenderResource) {
        let mut swap_chain_outputs = self.resources.swap_chain_outputs.write().unwrap();
        swap_chain_outputs.remove(&render_resource);
    }

    fn drop_all_swap_chain_textures(&self) {
        let mut swap_chain_outputs = self.resources.swap_chain_outputs.write().unwrap();
        swap_chain_outputs.clear();
    }

    fn set_asset_resource_untyped(
        &self,
        handle: HandleUntyped,
        render_resource: RenderResource,
        index: usize,
    ) {
        let mut asset_resources = self.resources.asset_resources.write().unwrap();
        asset_resources.insert((handle, index), render_resource);
    }

    fn get_asset_resource_untyped(
        &self,
        handle: HandleUntyped,
        index: usize,
    ) -> Option<RenderResource> {
        let asset_resources = self.resources.asset_resources.read().unwrap();
        asset_resources.get(&(handle, index)).cloned()
    }

    fn create_render_pipeline(
        &self,
        pipeline_handle: Handle<PipelineDescriptor>,
        pipeline_descriptor: &PipelineDescriptor,
        shaders: &AssetStorage<Shader>,
    ) {
        if self
            .resources
            .render_pipelines
            .read()
            .unwrap()
            .get(&pipeline_handle)
            .is_some()
        {
            return;
        }

        let layout = pipeline_descriptor.get_layout().unwrap();
        for bind_group_descriptor in layout.bind_groups.iter() {
            self.create_bind_group_layout(&bind_group_descriptor);
        }

        let bind_group_layouts = self.resources.bind_group_layouts.read().unwrap();
        // setup and collect bind group layouts
        let bind_group_layouts = layout
            .bind_groups
            .iter()
            .map(|bind_group| bind_group_layouts.get(&bind_group.id).unwrap())
            .collect::<Vec<&wgpu::BindGroupLayout>>();

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: bind_group_layouts.as_slice(),
            });

        let owned_vertex_buffer_descriptors = layout
            .vertex_buffer_descriptors
            .iter()
            .map(|v| v.wgpu_into())
            .collect::<Vec<OwnedWgpuVertexBufferDescriptor>>();

        let color_states = pipeline_descriptor
            .color_states
            .iter()
            .map(|c| c.wgpu_into())
            .collect::<Vec<wgpu::ColorStateDescriptor>>();

        self.create_shader_module(pipeline_descriptor.shader_stages.vertex, shaders);

        if let Some(fragment_handle) = pipeline_descriptor.shader_stages.fragment {
            self.create_shader_module(fragment_handle, shaders);
        }

        let shader_modules = self.resources.shader_modules.read().unwrap();
        let vertex_shader_module = shader_modules
            .get(&pipeline_descriptor.shader_stages.vertex)
            .unwrap();

        let fragment_shader_module = match pipeline_descriptor.shader_stages.fragment {
            Some(fragment_handle) => Some(shader_modules.get(&fragment_handle).unwrap()),
            None => None,
        };

        let render_pipeline_descriptor = wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vertex_shader_module,
                entry_point: "main",
            },
            fragment_stage: match pipeline_descriptor.shader_stages.fragment {
                Some(_) => Some(wgpu::ProgrammableStageDescriptor {
                    entry_point: "main",
                    module: fragment_shader_module.as_ref().unwrap(),
                }),
                None => None,
            },
            rasterization_state: pipeline_descriptor
                .rasterization_state
                .as_ref()
                .map(|r| r.wgpu_into()),
            primitive_topology: pipeline_descriptor.primitive_topology.wgpu_into(),
            color_states: &color_states,
            depth_stencil_state: pipeline_descriptor
                .depth_stencil_state
                .as_ref()
                .map(|d| d.wgpu_into()),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: pipeline_descriptor.index_format.wgpu_into(),
                vertex_buffers: &owned_vertex_buffer_descriptors
                    .iter()
                    .map(|v| v.into())
                    .collect::<Vec<wgpu::VertexBufferDescriptor>>(),
            },
            sample_count: pipeline_descriptor.sample_count,
            sample_mask: pipeline_descriptor.sample_mask,
            alpha_to_coverage_enabled: pipeline_descriptor.alpha_to_coverage_enabled,
        };

        let render_pipeline = self
            .device
            .create_render_pipeline(&render_pipeline_descriptor);
        let mut render_pipelines = self.resources.render_pipelines.write().unwrap();
        render_pipelines.insert(pipeline_handle, render_pipeline);
    }

    fn create_bind_group(
        &self,
        bind_group_descriptor: &BindGroupDescriptor,
        render_resource_assignments: &RenderResourceAssignments,
    ) -> Option<RenderResourceSetId> {
        if let Some(render_resource_set) =
            render_resource_assignments.get_render_resource_set(bind_group_descriptor.id)
        {
            if !self
                .resources
                .has_bind_group(bind_group_descriptor.id, render_resource_set.id)
            {
                log::trace!(
                    "start creating bind group for RenderResourceSet {:?}",
                    render_resource_set.id
                );
                let texture_views = self.resources.texture_views.read().unwrap();
                let samplers = self.resources.samplers.read().unwrap();
                let buffers = self.resources.buffers.read().unwrap();
                let bind_group_layouts = self.resources.bind_group_layouts.read().unwrap();
                let mut bind_groups = self.resources.bind_groups.write().unwrap();

                let bindings = bind_group_descriptor
                    .bindings
                    .iter()
                    .map(|binding| {
                        if let Some(assignment) = render_resource_assignments.get(&binding.name) {
                            log::trace!(
                                "found binding {} ({}) assignment: {:?}",
                                binding.index,
                                binding.name,
                                assignment,
                            );
                            let wgpu_resource = match assignment {
                                RenderResourceAssignment::Texture(resource) => {
                                    let texture = texture_views.get(&resource).unwrap();
                                    wgpu::BindingResource::TextureView(texture)
                                }
                                RenderResourceAssignment::Sampler(resource) => {
                                    let sampler = samplers.get(&resource).unwrap();
                                    wgpu::BindingResource::Sampler(sampler)
                                }
                                RenderResourceAssignment::Buffer { resource, range , .. } => {
                                    let buffer = buffers.get(&resource).unwrap();
                                    wgpu::BindingResource::Buffer {
                                        buffer,
                                        range: range.clone(),
                                    }
                                }
                            };
                            wgpu::Binding {
                                binding: binding.index,
                                resource: wgpu_resource,
                            }
                        } else {
                            panic!(
                                "No resource assigned to uniform \"{}\" for RenderResourceAssignments {:?}",
                                binding.name,
                                render_resource_assignments.id
                            );
                        }
                    })
                    .collect::<Vec<wgpu::Binding>>();

                let bind_group_layout = bind_group_layouts.get(&bind_group_descriptor.id).unwrap();
                let wgpu_bind_group_descriptor = wgpu::BindGroupDescriptor {
                    label: None,
                    layout: bind_group_layout,
                    bindings: bindings.as_slice(),
                };
                let wgpu_bind_group = self.device.create_bind_group(&wgpu_bind_group_descriptor);

                let bind_group_info = bind_groups
                    .entry(bind_group_descriptor.id)
                    .or_insert_with(|| WgpuBindGroupInfo::default());
                bind_group_info
                    .bind_groups
                    .insert(render_resource_set.id, wgpu_bind_group);
                log::trace!(
                    "created bind group for RenderResourceSet {:?}",
                    render_resource_set.id
                );
                return Some(render_resource_set.id);
            }
        }

        None
    }
}