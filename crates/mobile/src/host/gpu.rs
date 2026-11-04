use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use glow::HasContext;
use slint::{BorrowedOpenGLTextureBuilder, BorrowedOpenGLTextureOrigin, ComponentHandle, GraphicsAPI, Image, RenderingState};
use tracing::{info, warn};

use foundry::systems::raster::Vertex;

use crate::{AppWindow, Sandbox};

use super::viewport::Canvas;

const VERTEX_SHADER: &str = "#version 300 es
layout(location = 0) in vec2 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 color;
out vec2 v_uv;
out vec4 v_color;
void main() {
    v_uv = uv;
    v_color = color;
    gl_Position = vec4(position, 0.0, 1.0);
}
";
const FRAGMENT_SHADER: &str = "#version 300 es
precision highp float;
uniform sampler2D sheet;
in vec2 v_uv;
in vec4 v_color;
out vec4 result;
void main() {
    result = texture(sheet, v_uv) * v_color;
}
";
const RESOLVE_VERTEX_SHADER: &str = "#version 300 es
layout(location = 0) in vec2 corner;
out vec2 v_uv;
void main() {
    v_uv = corner;
    gl_Position = vec4(corner * 2.0 - 1.0, 0.0, 1.0);
}
";
const RESOLVE_FRAGMENT_SHADER: &str = "#version 300 es
precision highp float;
uniform sampler2D sheet;
in vec2 v_uv;
out vec4 result;
void main() {
    vec4 texel = texture(sheet, v_uv);
    result = texel.a > 0.0 ? vec4(texel.rgb / texel.a, texel.a) : vec4(0.0);
}
";
const WHITE: [u8; 4] = [0xff; 4];
const LEAST_GLES: u32 = 3;
const POSITION: u32 = 0;
const UV: u32 = 1;
const COLOR: u32 = 2;
const STRIDE: i32 = size_of::<Vertex>() as i32;
const UV_OFFSET: i32 = 2 * size_of::<f32>() as i32;
const COLOR_OFFSET: i32 = 4 * size_of::<f32>() as i32;
const RESOLVE_TRIANGLE: [f32; 6] = [0.0, 0.0, 2.0, 0.0, 0.0, 2.0];
const RESOLVE_CORNERS: i32 = 3;

static AVAILABLE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static PENDING: RefCell<Option<Scene>> = const { RefCell::new(None) };
    static FORGET: Cell<bool> = const { Cell::new(false) };
}

pub struct SceneSheet {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixels: Arc<[u8]>,
}

pub struct SceneRun {
    pub(super) sheet: Option<usize>,
    pub(super) blend: u8,
    pub(super) range: Range<usize>,
}

pub struct Scene {
    pub(super) vertices: Vec<Vertex>,
    pub(super) runs: Vec<SceneRun>,
    pub(super) sheets: Vec<SceneSheet>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub canvas: Canvas,
}

pub fn available() -> bool {
    AVAILABLE.load(Ordering::Acquire)
}

pub fn submit(scene: Scene) {
    PENDING.with(|pending| *pending.borrow_mut() = Some(scene));
}

pub fn forget() {
    PENDING.with(|pending| *pending.borrow_mut() = None);
    FORGET.with(|forget| forget.set(true));
}

struct Target {
    framebuffer: glow::Framebuffer,
    texture: glow::Texture,
    width: u32,
    height: u32,
}

struct Saved {
    framebuffer: Option<glow::Framebuffer>,
    read_framebuffer: Option<glow::Framebuffer>,
    unpack_buffer: Option<glow::Buffer>,
    sampler: Option<glow::Sampler>,
    viewport: [i32; 4],
    program: Option<glow::Program>,
    vertex_array: Option<glow::VertexArray>,
    buffer: Option<glow::Buffer>,
    active_texture: i32,
    texture: Option<glow::Texture>,
    blend: bool,
    scissor: bool,
    depth: bool,
    stencil: bool,
    cull: bool,
    blend_source: [i32; 2],
    blend_target: [i32; 2],
    blend_equation: [i32; 2],
    color_mask: [bool; 4],
    clear_color: [f32; 4],
    unpack_alignment: i32,
    unpack_row_length: i32,
}

struct Objects {
    program: glow::Program,
    resolve: glow::Program,
    vertex_array: glow::VertexArray,
    resolve_array: glow::VertexArray,
    buffer: glow::Buffer,
    resolve_buffer: glow::Buffer,
    white: glow::Texture,
}

struct Renderer {
    gl: glow::Context,
    objects: Objects,
    sheets: HashMap<usize, (Arc<[u8]>, glow::Texture)>,
    drawn: Option<Target>,
    shown: Option<Target>,
}

fn compile(gl: &glow::Context, vertex: &str, fragment: &str) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program()?;
        let mut stages = Vec::new();

        for (kind, source) in [(glow::VERTEX_SHADER, vertex), (glow::FRAGMENT_SHADER, fragment)] {
            let shader = gl.create_shader(kind)?;

            gl.shader_source(shader, source);
            gl.compile_shader(shader);

            if !gl.get_shader_compile_status(shader) {
                return Err(gl.get_shader_info_log(shader));
            }

            gl.attach_shader(program, shader);
            stages.push(shader);
        }

        gl.link_program(program);

        for shader in stages {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }

        let sheet = gl.get_uniform_location(program, "sheet");

        gl.use_program(Some(program));
        gl.uniform_1_i32(sheet.as_ref(), 0);

        Ok(program)
    }
}

fn texture(gl: &glow::Context, width: u32, height: u32, pixels: Option<&[u8]>) -> Result<glow::Texture, String> {
    unsafe {
        let texture = gl.create_texture()?;

        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(pixels),
        );

        Ok(texture)
    }
}

fn target(gl: &glow::Context, width: u32, height: u32) -> Result<Target, String> {
    unsafe {
        let texture = texture(gl, width, height, None)?;
        let framebuffer = gl.create_framebuffer()?;

        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(texture), 0);

        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);

        if status != glow::FRAMEBUFFER_COMPLETE {
            gl.delete_framebuffer(framebuffer);
            gl.delete_texture(texture);

            return Err(format!("the battle framebuffer is incomplete: {status:#x}"));
        }

        Ok(Target { framebuffer, texture, width, height })
    }
}

fn blending(gl: &glow::Context, mode: u8) {
    let (source, destination) = match mode {
        1 => (glow::ONE, glow::ONE),
        2 => (glow::DST_COLOR, glow::ZERO),
        3 => (glow::ONE, glow::ONE_MINUS_SRC_COLOR),
        _ => (glow::ONE, glow::ONE_MINUS_SRC_ALPHA),
    };
    let (source_alpha, destination_alpha) = if (1..=3).contains(&mode) { (glow::ZERO, glow::ONE) } else { (glow::ONE, glow::ONE_MINUS_SRC_ALPHA) };

    unsafe {
        gl.blend_func_separate(source, destination, source_alpha, destination_alpha);
    }
}

impl Saved {
    fn take(gl: &glow::Context) -> Self {
        unsafe {
            let mut viewport = [0; 4];

            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut viewport);

            let mut clear_color = [0.0; 4];

            gl.get_parameter_f32_slice(glow::COLOR_CLEAR_VALUE, &mut clear_color);

            let active_texture = gl.get_parameter_i32(glow::ACTIVE_TEXTURE);

            gl.active_texture(glow::TEXTURE0);

            Self {
                framebuffer: gl.get_parameter_framebuffer(glow::DRAW_FRAMEBUFFER_BINDING),
                read_framebuffer: gl.get_parameter_framebuffer(glow::READ_FRAMEBUFFER_BINDING),
                unpack_buffer: gl.get_parameter_buffer(glow::PIXEL_UNPACK_BUFFER_BINDING),
                sampler: gl.get_parameter_sampler(glow::SAMPLER_BINDING),
                viewport,
                program: gl.get_parameter_program(glow::CURRENT_PROGRAM),
                vertex_array: gl.get_parameter_vertex_array(glow::VERTEX_ARRAY_BINDING),
                buffer: gl.get_parameter_buffer(glow::ARRAY_BUFFER_BINDING),
                active_texture,
                texture: gl.get_parameter_texture(glow::TEXTURE_BINDING_2D),
                blend: gl.is_enabled(glow::BLEND),
                scissor: gl.is_enabled(glow::SCISSOR_TEST),
                depth: gl.is_enabled(glow::DEPTH_TEST),
                stencil: gl.is_enabled(glow::STENCIL_TEST),
                cull: gl.is_enabled(glow::CULL_FACE),
                blend_source: [gl.get_parameter_i32(glow::BLEND_SRC_RGB), gl.get_parameter_i32(glow::BLEND_SRC_ALPHA)],
                blend_target: [gl.get_parameter_i32(glow::BLEND_DST_RGB), gl.get_parameter_i32(glow::BLEND_DST_ALPHA)],
                blend_equation: [gl.get_parameter_i32(glow::BLEND_EQUATION_RGB), gl.get_parameter_i32(glow::BLEND_EQUATION_ALPHA)],
                color_mask: gl.get_parameter_bool_array::<4>(glow::COLOR_WRITEMASK),
                clear_color,
                unpack_alignment: gl.get_parameter_i32(glow::UNPACK_ALIGNMENT),
                unpack_row_length: gl.get_parameter_i32(glow::UNPACK_ROW_LENGTH),
            }
        }
    }

    fn restore(&self, gl: &glow::Context) {
        let toggle = |capability: u32, on: bool| unsafe {
            if on {
                gl.enable(capability);
            } else {
                gl.disable(capability);
            }
        };

        toggle(glow::BLEND, self.blend);
        toggle(glow::SCISSOR_TEST, self.scissor);
        toggle(glow::DEPTH_TEST, self.depth);
        toggle(glow::STENCIL_TEST, self.stencil);
        toggle(glow::CULL_FACE, self.cull);

        unsafe {
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, self.framebuffer);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, self.read_framebuffer);
            gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, self.unpack_buffer);
            gl.bind_sampler(0, self.sampler);
            gl.viewport(self.viewport[0], self.viewport[1], self.viewport[2], self.viewport[3]);
            gl.use_program(self.program);
            gl.bind_vertex_array(self.vertex_array);
            gl.bind_buffer(glow::ARRAY_BUFFER, self.buffer);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, self.texture);
            gl.active_texture(self.active_texture as u32);
            gl.blend_func_separate(self.blend_source[0] as u32, self.blend_target[0] as u32, self.blend_source[1] as u32, self.blend_target[1] as u32);
            gl.blend_equation_separate(self.blend_equation[0] as u32, self.blend_equation[1] as u32);
            gl.color_mask(self.color_mask[0], self.color_mask[1], self.color_mask[2], self.color_mask[3]);
            gl.clear_color(self.clear_color[0], self.clear_color[1], self.clear_color[2], self.clear_color[3]);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, self.unpack_alignment);
            gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, self.unpack_row_length);
        }
    }
}

impl Renderer {
    fn new(gl: glow::Context) -> Result<Self, String> {
        let version = gl.version();

        if !version.is_embedded || version.major < LEAST_GLES {
            return Err(format!("the battle needs OpenGL ES {LEAST_GLES}, this context is {}.{}", version.major, version.minor));
        }

        let saved = Saved::take(&gl);
        let built = Self::build(&gl);

        saved.restore(&gl);

        Ok(Self { objects: built?, gl, sheets: HashMap::new(), drawn: None, shown: None })
    }

    fn build(gl: &glow::Context) -> Result<Objects, String> {
        unsafe {
            gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            gl.bind_sampler(0, None);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);

            let program = compile(gl, VERTEX_SHADER, FRAGMENT_SHADER)?;
            let resolve = compile(gl, RESOLVE_VERTEX_SHADER, RESOLVE_FRAGMENT_SHADER)?;
            let vertex_array = gl.create_vertex_array()?;
            let resolve_array = gl.create_vertex_array()?;
            let buffer = gl.create_buffer()?;
            let resolve_buffer = gl.create_buffer()?;

            gl.bind_vertex_array(Some(vertex_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
            gl.enable_vertex_attrib_array(POSITION);
            gl.vertex_attrib_pointer_f32(POSITION, 2, glow::FLOAT, false, STRIDE, 0);
            gl.enable_vertex_attrib_array(UV);
            gl.vertex_attrib_pointer_f32(UV, 2, glow::FLOAT, false, STRIDE, UV_OFFSET);
            gl.enable_vertex_attrib_array(COLOR);
            gl.vertex_attrib_pointer_f32(COLOR, 4, glow::FLOAT, false, STRIDE, COLOR_OFFSET);
            gl.bind_vertex_array(Some(resolve_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(resolve_buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&RESOLVE_TRIANGLE), glow::STATIC_DRAW);
            gl.enable_vertex_attrib_array(POSITION);
            gl.vertex_attrib_pointer_f32(POSITION, 2, glow::FLOAT, false, 0, 0);

            let white = texture(gl, 1, 1, Some(&WHITE))?;

            Ok(Objects { program, resolve, vertex_array, resolve_array, buffer, resolve_buffer, white })
        }
    }

    fn forget(&mut self) {
        unsafe {
            for (_, (_, texture)) in self.sheets.drain() {
                self.gl.delete_texture(texture);
            }
        }

        for held in [self.drawn.take(), self.shown.take()].into_iter().flatten() {
            self.release(&held);
        }
    }

    fn release(&self, held: &Target) {
        unsafe {
            self.gl.delete_framebuffer(held.framebuffer);
            self.gl.delete_texture(held.texture);
        }
    }

    fn sized(&mut self, width: u32, height: u32) -> Result<(), String> {
        let current = self.drawn.as_ref().is_some_and(|held| held.width == width && held.height == height);

        if current {
            return Ok(());
        }

        for held in [self.drawn.take(), self.shown.take()].into_iter().flatten() {
            self.release(&held);
        }

        self.drawn = Some(target(&self.gl, width, height)?);
        self.shown = Some(target(&self.gl, width, height)?);

        Ok(())
    }

    fn sheet(&mut self, sheet: &SceneSheet) -> Result<glow::Texture, String> {
        let key = Arc::as_ptr(&sheet.pixels).cast::<u8>() as usize;

        if let Some((_, texture)) = self.sheets.get(&key) {
            return Ok(*texture);
        }

        let made = texture(&self.gl, sheet.width, sheet.height, Some(&sheet.pixels))?;

        self.sheets.insert(key, (Arc::clone(&sheet.pixels), made));

        Ok(made)
    }

    fn draw(&mut self, scene: &Scene) -> Result<(), String> {
        unsafe {
            self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            self.gl.bind_sampler(0, None);
            self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            self.gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
        }

        self.sized(scene.width, scene.height)?;

        let drawn = self.drawn.as_ref().map(|held| held.framebuffer);
        let shown = self.shown.as_ref().map(|held| held.framebuffer);
        let source = self.drawn.as_ref().map(|held| held.texture);
        let bindings: Vec<Option<glow::Texture>> = scene
            .runs
            .iter()
            .map(|run| run.sheet.and_then(|index| scene.sheets.get(index)).map(|sheet| self.sheet(sheet)).transpose())
            .collect::<Result<_, _>>()?;
        let (width, height) = (scene.width as i32, scene.height as i32);
        let gl = &self.gl;

        unsafe {
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::STENCIL_TEST);
            gl.disable(glow::CULL_FACE);
            gl.color_mask(true, true, true, true);
            gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);
            gl.viewport(0, 0, width, height);
            gl.bind_framebuffer(glow::FRAMEBUFFER, drawn);
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.enable(glow::BLEND);
            gl.use_program(Some(self.objects.program));
            gl.bind_vertex_array(Some(self.objects.vertex_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.objects.buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&scene.vertices), glow::STREAM_DRAW);
            gl.active_texture(glow::TEXTURE0);

            for (run, binding) in scene.runs.iter().zip(&bindings) {
                gl.bind_texture(glow::TEXTURE_2D, Some(binding.unwrap_or(self.objects.white)));
                blending(gl, run.blend);
                gl.draw_arrays(glow::TRIANGLES, run.range.start as i32, run.range.len() as i32);
            }

            gl.disable(glow::BLEND);
            gl.bind_framebuffer(glow::FRAMEBUFFER, shown);
            gl.use_program(Some(self.objects.resolve));
            gl.bind_vertex_array(Some(self.objects.resolve_array));
            gl.bind_texture(glow::TEXTURE_2D, source);
            gl.draw_arrays(glow::TRIANGLES, 0, RESOLVE_CORNERS);
        }

        Ok(())
    }

    fn render(&mut self, scene: &Scene) -> Option<(glow::Texture, u32, u32)> {
        let saved = Saved::take(&self.gl);
        let drawn = self.draw(scene);

        saved.restore(&self.gl);

        if let Err(err) = drawn {
            warn!("emu: the battle frame could not be drawn: {err}");

            return None;
        }

        self.shown.as_ref().map(|held| (held.texture, held.width, held.height))
    }

    fn dispose(mut self) {
        self.forget();

        unsafe {
            self.gl.delete_program(self.objects.program);
            self.gl.delete_program(self.objects.resolve);
            self.gl.delete_vertex_array(self.objects.vertex_array);
            self.gl.delete_vertex_array(self.objects.resolve_array);
            self.gl.delete_buffer(self.objects.buffer);
            self.gl.delete_buffer(self.objects.resolve_buffer);
            self.gl.delete_texture(self.objects.white);
        }
    }
}

#[derive(Default)]
struct Presenter {
    renderer: Option<Renderer>,
    latest: Option<Scene>,
    fresh: bool,
    bound: Option<(glow::Texture, u32, u32)>,
}

impl Presenter {
    fn setup(&mut self, api: &GraphicsAPI<'_>) {
        let GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
            info!("emu: this renderer has no OpenGL context, battles draw on the CPU");

            return;
        };
        let gl = unsafe { glow::Context::from_loader_function_cstr(|name| get_proc_address(name)) };

        match Renderer::new(gl) {
            Ok(renderer) => {
                info!("emu: battles draw on the GPU");
                self.renderer = Some(renderer);
                self.fresh = self.latest.is_some();
                self.bound = None;
                AVAILABLE.store(true, Ordering::Release);
            }
            Err(err) => {
                AVAILABLE.store(false, Ordering::Release);
                warn!("emu: battles fall back to the CPU: {err}");
            }
        }
    }

    fn teardown(&mut self) {
        self.bound = None;

        if let Some(renderer) = self.renderer.take() {
            renderer.dispose();
        }
    }

    fn present(&mut self, ui: &AppWindow) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        if FORGET.with(|forget| forget.take()) {
            self.latest = None;
            self.fresh = false;
            self.bound = None;
            let saved = Saved::take(&renderer.gl);

            renderer.forget();
            saved.restore(&renderer.gl);
        }

        if let Some(scene) = PENDING.with(|pending| pending.borrow_mut().take()) {
            self.latest = Some(scene);
            self.fresh = true;
        }

        if !std::mem::take(&mut self.fresh) {
            return;
        }

        let Some(rendered) = self.latest.as_ref().and_then(|scene| renderer.render(scene)) else {
            return;
        };

        if self.bound != Some(rendered) {
            let (texture, width, height) = rendered;
            let image: Image = unsafe {
                BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(texture.0, (width, height).into()).origin(BorrowedOpenGLTextureOrigin::BottomLeft).build()
            };

            ui.global::<Sandbox>().set_frame(image);
            self.bound = Some(rendered);
        }
    }
}

pub fn install(window: &AppWindow) {
    let weak = window.as_weak();
    let mut presenter = Presenter::default();
    let outcome = window.window().set_rendering_notifier(move |state, api| match state {
        RenderingState::RenderingSetup => presenter.setup(api),
        RenderingState::BeforeRendering => {
            if let Some(ui) = weak.upgrade() {
                presenter.present(&ui);
            }
        }
        RenderingState::RenderingTeardown => presenter.teardown(),
        _ => {}
    });

    if let Err(err) = outcome {
        warn!("emu: battles draw on the CPU, the renderer takes no notifier: {err}");
    }
}
