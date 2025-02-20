use std::{cell::RefCell, ops::DerefMut, rc::Rc};

use roots_core::{
    common::Size,
    prelude::{
        camera::{Camera, OrthographicCamera},
        Color, Device, Queue, Surface, SurfaceConfig,
    },
    renderer::{
        shared::{SharedRenderResources, Vertex},
        tools, RenderCore, RenderEncoder, RenderPass, RenderPassDesc,
    },
    runner::window::Window,
};

pub struct Renderer {
    pub device: Device,
    pub queue: Queue,
    config: SurfaceConfig,
    surface: Surface<'static>,

    _shared: SharedRenderResources,

    pub circle_pipeline: CirclePipeline,
    pub polygon_pipeline: PolygonPipeline,
    pub render_circles: bool,
    pub render_polygons: bool,

    pub clear_color: Color,
    camera_data: OrthographicCamera,
    camera: Camera,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
        let (device, queue, surface, config) =
            RenderCore::new_blocked(window.clone_arc(), window.size())
                .unwrap()
                .break_down();

        let shared = SharedRenderResources::new(&device);
        let circle_pipeline = CirclePipeline::new(&device, &config, &shared);
        let polygon_pipeline = PolygonPipeline::new(&device, &config, &shared);

        // let camera_data = OrthographicCamera::new_sized(1920., 1080.);
        let camera_data = OrthographicCamera::new_centered(1920. / 2., 1080. / 2.);
        let camera = Camera::new(&device, &camera_data, shared.camera_bind_group_layout());

        Self {
            device,
            queue,
            config,
            surface,

            _shared: shared,
            circle_pipeline,
            polygon_pipeline,
            render_circles: true,
            render_polygons: true,

            clear_color: Color::new(0.1, 0.1, 0.1, 1.),
            camera_data,
            camera,
        }
    }

    pub fn resize(&mut self, size: Size<u32>) {
        log::debug!("Resizing window with new size {}", size);
        self.config.width = size.width;
        self.config.height = size.height;

        self.surface.configure(&self.device, &self.config);

        // self.camera_data
        //     .set_size(size.width as f32, size.height as f32);
        self.camera_data
            .set_size_centered(size.width as f32 / 2., size.height as f32 / 2.);

        self.camera
            .update_camera(&self.queue, &self.camera_data, &glam::Affine3A::IDENTITY);
    }

    pub fn prep(&mut self) {
        self.circle_pipeline.finish_prep(&self.device, &self.queue);
        self.polygon_pipeline.finish_prep();
    }

    pub fn render(&self) {
        let mut encoder = RenderEncoder::new(&self.device, &self.surface).unwrap();

        let mut render_pass = encoder.begin_render_pass(RenderPassDesc {
            use_depth: None,
            clear_color: Some(self.clear_color),
        });

        if self.render_circles {
            self.circle_pipeline
                .render(&mut render_pass, self.camera.bind_group());
        }

        if self.render_polygons {
            self.polygon_pipeline
                .render(&mut render_pass, self.camera.bind_group());
        }

        render_pass.drop();
        encoder.finish(&self.queue);
    }
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub struct RawVertex {
    pub pos: glam::Vec2,
}

impl Vertex for RawVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![
            0 => Float32x2
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RawVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

const RECT_VERTICES: [RawVertex; 4] = [
    RawVertex {
        pos: glam::vec2(-0.5, 0.5),
    },
    RawVertex {
        pos: glam::vec2(-0.5, -0.5),
    },
    RawVertex {
        pos: glam::vec2(0.5, 0.5),
    },
    RawVertex {
        pos: glam::vec2(0.5, -0.5),
    },
];

pub const RECT_INDICES: [u16; 6] = [0, 1, 3, 0, 3, 2];

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct CircleInstance {
    pos: glam::Vec2,
    radius: f32,
    border_radius: f32,
    color: glam::Vec4,
    border_color: glam::Vec4,
}

impl Vertex for CircleInstance {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            1 => Float32x2,
            2 => Float32,
            3 => Float32,
            4 => Float32x4,
            5 => Float32x4,
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CircleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

impl CircleInstance {
    pub fn new(pos: impl Into<glam::Vec2>, radius: f32) -> Self {
        Self {
            pos: pos.into(),
            radius,
            border_radius: 6.,
            color: glam::Vec4::ONE,
            border_color: glam::vec4(0., 0., 0., 1.),
        }
    }
    pub fn with_color(mut self, color: glam::Vec4) -> Self {
        self.color = color;
        self
    }
    pub fn hollow(mut self) -> Self {
        self.color = glam::Vec4::ZERO;
        self
    }
    pub fn with_border(mut self, radius: f32, color: glam::Vec4) -> Self {
        self.border_radius = radius;
        self.border_color = color;
        self
    }
}

pub struct CirclePipeline {
    pipeline: wgpu::RenderPipeline,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,

    instance_buffer: wgpu::Buffer,
    instance_count: u32,

    to_prep: Vec<CircleInstance>,
}

impl CirclePipeline {
    pub fn new(device: &Device, config: &SurfaceConfig, shared: &SharedRenderResources) -> Self {
        let pipeline = tools::create_pipeline(
            device,
            config,
            "Circle Pipeline",
            &[shared.camera_bind_group_layout()],
            &[RawVertex::desc(), CircleInstance::desc()],
            include_str!("circle_shader.wgsl").into(),
            tools::RenderPipelineDescriptor::default(),
        );

        let vertex_buffer = tools::create_buffer(
            device,
            tools::BufferType::Vertex,
            "Circle Pipeline",
            &RECT_VERTICES,
        );

        let index_buffer = tools::create_buffer(
            device,
            tools::BufferType::Index,
            "Circle Pipeline",
            &RECT_INDICES,
        );

        let index_count = RECT_INDICES.len() as u32;

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Circle Pipeline Instance Buffer"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let instance_count = 0;

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            instance_buffer,
            instance_count,
            to_prep: Vec::new(),
        }
    }

    #[inline]
    pub fn prep_circle(&mut self, circle: CircleInstance) {
        self.to_prep.push(circle);
    }

    #[inline]
    pub fn finish_prep(&mut self, device: &Device, queue: &Queue) {
        tools::update_buffer_data(
            device,
            queue,
            tools::BufferType::Instance,
            "Cirle Pipeline",
            &mut self.instance_buffer,
            &mut self.instance_count,
            &self.to_prep,
        );

        self.to_prep.clear();
    }

    pub fn render(&self, pass: &mut RenderPass, camera_bind_group: &wgpu::BindGroup) {
        if self.instance_count == 0 {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);

        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

        pass.draw_indexed(0..self.index_count, 0, 0..self.instance_count);
    }
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub struct PolygonVertex {
    pub pos: glam::Vec2,
    pub pad: [u32; 2],
    pub color: glam::Vec4,
}

impl Vertex for PolygonVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
            0 => Float32x4,
            1 => Float32x4
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PolygonVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}

#[derive(Clone)]
pub struct PolygonInstance(Rc<RefCell<PolygonInstanceInner>>);

pub struct PolygonInstanceInner {
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl PolygonInstance {
    pub fn update(
        &mut self,
        device: &Device,
        queue: &Queue,
        vertices: &[PolygonVertex],
        indices: &[u16],
    ) {
        let mut inner = self.0.borrow_mut();

        let PolygonInstanceInner {
            vertex_buffer,
            vertex_count,
            index_buffer,
            index_count,
        } = inner.deref_mut();

        tools::update_buffer_data(
            device,
            queue,
            tools::BufferType::VertexDynamic,
            "Polygon",
            vertex_buffer,
            vertex_count,
            vertices,
        );

        tools::update_buffer_data(
            device,
            queue,
            tools::BufferType::IndexDynamic,
            "Polygon",
            index_buffer,
            index_count,
            indices,
        );
    }
}

pub struct PolygonPipeline {
    pipeline: wgpu::RenderPipeline,
    instances: Vec<PolygonInstance>,
}

impl PolygonPipeline {
    pub fn new(device: &Device, config: &SurfaceConfig, shared: &SharedRenderResources) -> Self {
        let pipeline = tools::create_pipeline(
            device,
            config,
            "Polygon Pipeline",
            &[shared.camera_bind_group_layout()],
            &[PolygonVertex::desc()],
            include_str!("polygon_shader.wgsl").into(),
            tools::RenderPipelineDescriptor::default(),
        );

        Self {
            pipeline,
            instances: Vec::new(),
        }
    }

    pub fn new_polygon(
        &mut self,
        device: &Device,
        vertices: &[PolygonVertex],
        indices: &[u16],
    ) -> PolygonInstance {
        let vertex_buffer = tools::create_buffer(
            device,
            tools::BufferType::VertexDynamic,
            "Polygon",
            vertices,
        );
        let index_buffer =
            tools::create_buffer(device, tools::BufferType::IndexDynamic, "Polygon", indices);

        let instance = PolygonInstance(Rc::new(RefCell::new(PolygonInstanceInner {
            vertex_buffer,
            vertex_count: vertices.len() as u32,
            index_buffer,
            index_count: indices.len() as u32,
        })));

        self.instances.push(instance.clone());

        instance
    }

    pub fn finish_prep(&mut self) {
        // Remove all instances with only one reference
        self.instances
            .retain(|instance| Rc::strong_count(&instance.0) > 1);
    }

    pub fn render(&self, pass: &mut RenderPass, camera_bind_group: &wgpu::BindGroup) {
        if self.instances.len() == 0 {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);

        self.instances.iter().for_each(|instance| {
            let instance = instance.0.borrow();

            pass.set_vertex_buffer(0, instance.vertex_buffer.slice(..));
            pass.set_index_buffer(instance.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..instance.index_count, 0, 0..1);
        });
    }
}
