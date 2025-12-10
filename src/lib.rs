mod camera;
mod input;
mod particles;
mod presets;
mod sky;
mod terrain;
mod utils;
mod webgpu;

use std::cell::RefCell;
use std::rc::Rc;
use js_sys::Math;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use camera::FlyCamera;
use input::InputState;
use particles::{ParticleSettings, ParticleSystem};
use sky::{SkyRenderer, SkySettings};
use terrain::{TerrainRenderer, TerrainSettings};
use webgpu::GpuState;

// Global state for JS access
thread_local! {
    static APP_STATE: RefCell<Option<Rc<RefCell<AppState>>>> = RefCell::new(None);
}

/// Helper to access APP_STATE with mutable access and automatic error handling
fn with_app_state_mut<F, T>(f: F) -> Result<T, JsValue>
where
    F: FnOnce(&mut AppState) -> T,
{
    APP_STATE.with(|s| {
        if let Some(state) = s.borrow().as_ref() {
            Ok(f(&mut state.borrow_mut()))
        } else {
            Err(JsValue::from_str("App not initialized"))
        }
    })
}

/// Helper to access APP_STATE with read-only access and automatic error handling
fn with_app_state<F, T>(f: F) -> Result<T, JsValue>
where
    F: FnOnce(&AppState) -> T,
{
    APP_STATE.with(|s| {
        if let Some(state) = s.borrow().as_ref() {
            Ok(f(&state.borrow()))
        } else {
            Err(JsValue::from_str("App not initialized"))
        }
    })
}

/// Main application state
pub struct AppState {
    gpu: GpuState,
    camera: FlyCamera,
    input: InputState,
    terrain: TerrainRenderer,
    sky: SkyRenderer,
    particles: ParticleSystem,
    last_time: f64,
}

impl AppState {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let width = canvas.width();
        let height = canvas.height();

        let gpu = GpuState::new(&canvas).await?;
        let camera = FlyCamera::new(width as f32 / height as f32);
        let input = InputState::new();

        // Get preset settings first so terrain is generated with correct settings
        let preset = presets::get_default_preset();

        // Create terrain renderer with correct initial settings
        let mut terrain_settings = preset.as_ref().map(|p| p.terrain.clone()).unwrap_or_default();
        // Randomize seed like clicking a preset button
        terrain_settings.seed = (Math::random() * 1000000.0) as u32;
        let terrain = TerrainRenderer::new(&gpu.device, &gpu.queue, gpu.surface_format, terrain_settings)?;

        let mut sky = SkyRenderer::new(&gpu.device, gpu.surface_format)?;
        let mut particles = ParticleSystem::new(&gpu.device, gpu.surface_format)?;

        // Apply sky and particle settings
        if let Some(preset) = preset {
            sky.update_settings(preset.sky);
            particles.update_settings(preset.particles);
        }

        Ok(Self {
            gpu,
            camera,
            input,
            terrain,
            sky,
            particles,
            last_time: 0.0,
        })
    }

    pub fn update(&mut self, current_time: f64) -> f32 {
        let dt = if self.last_time > 0.0 {
            ((current_time - self.last_time) / 1000.0) as f32
        } else {
            0.016
        };
        self.last_time = current_time;

        // Update camera based on input
        self.camera.update(&self.input, dt);

        // Clear per-frame input state
        self.input.clear_frame_state();

        // Check if terrain needs regeneration (settings changed or R key pressed)
        self.terrain
            .check_regeneration(&self.gpu.device, &self.gpu.queue, self.camera.position);

        // Update terrain chunks based on camera position
        self.terrain
            .update(&self.gpu.device, &self.gpu.queue, self.camera.position);

        // Update sky (animations, regeneration check)
        self.sky.update(dt);
        self.sky.check_regeneration();

        // Return dt for use in render (particles need it)
        dt
    }

    pub fn render(&mut self, dt: f32) {
        let output = match self.gpu.surface.get_current_texture() {
            Ok(output) => output,
            Err(e) => match e {
                wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                    self.gpu.resize(self.gpu.config.width, self.gpu.config.height);
                    return;
                }
                wgpu::SurfaceError::Timeout => {
                    log::warn!("Surface timeout, skipping frame");
                    return;
                }
                wgpu::SurfaceError::OutOfMemory => {
                    log::error!("Surface out of memory, stopping render loop");
                    return;
                }
            },
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let view_proj = self.camera.view_projection_matrix().to_cols_array_2d();
        let camera_pos = self.camera.position;

        // Create a SINGLE encoder for both compute and render passes
        // This ensures proper GPU command ordering - compute finishes before render reads
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Compute and Render Encoder"),
            });

        // FIRST: Run particle compute pass (updates particle positions)
        // This must happen before render passes that read particle data
        self.particles
            .update(&mut encoder, &self.gpu.queue, self.camera.position, dt);

        // Run terrain rendering (clears to sky horizon color)
        self.terrain.render(
            &mut encoder,
            &view,
            &self.gpu.depth_view,
            &self.camera,
            &self.gpu.queue,
        );

        // Render sky objects (no depth test, blends on top of sky areas)
        self.sky.render(
            &mut encoder,
            &view,
            view_proj,
            camera_pos,
            &self.gpu.queue,
        );

        // Render particles (with depth read, after terrain)
        // Now reads from the buffer that compute just wrote to
        self.particles.render(
            &mut encoder,
            &view,
            &self.gpu.depth_view,
            view_proj,
            camera_pos,
            &self.gpu.queue,
        );

        // Submit all commands together - GPU executes them in order
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.gpu.resize(width, height);
            self.camera.aspect = width as f32 / height as f32;
        }
    }

    pub fn input_mut(&mut self) -> &mut InputState {
        &mut self.input
    }

    pub fn update_terrain_settings(&mut self, settings: TerrainSettings) {
        self.terrain.update_settings(settings);
    }

    pub fn get_terrain_settings(&self) -> &TerrainSettings {
        &self.terrain.settings
    }

    pub fn queue_terrain_regeneration(&mut self) {
        self.terrain.queue_regeneration();
    }

    pub fn update_sky_settings(&mut self, settings: SkySettings) {
        self.sky.update_settings(settings);
    }

    pub fn get_sky_settings(&self) -> &SkySettings {
        &self.sky.settings
    }

    pub fn update_particle_settings(&mut self, settings: ParticleSettings) {
        self.particles.update_settings(settings);
    }

    pub fn get_particle_settings(&self) -> &ParticleSettings {
        &self.particles.settings
    }
}

#[wasm_bindgen(start)]
pub async fn run() {
    utils::init();

    log::info!("Starting Procedural Terrain Simulator...");

    // Get canvas from DOM
    let window = web_sys::window().expect("No window");
    let document = window.document().expect("No document");
    let canvas = document
        .get_element_by_id("canvas")
        .expect("No canvas element with id 'canvas'")
        .dyn_into::<HtmlCanvasElement>()
        .expect("Element is not a canvas");

    // Initialize app state
    let state = match AppState::new(canvas.clone()).await {
        Ok(state) => Rc::new(RefCell::new(state)),
        Err(e) => {
            log::error!("Failed to initialize: {}", e);
            show_error(&document, &format!("WebGPU initialization failed: {}", e));
            return;
        }
    };

    // Store state globally for JS access
    APP_STATE.with(|s| {
        *s.borrow_mut() = Some(Rc::clone(&state));
    });

    // Setup input handlers
    input::setup_input_handlers(&canvas, Rc::clone(&state));

    // Setup resize handler
    setup_resize_handler(&canvas, Rc::clone(&state));

    // Start the frame loop
    start_frame_loop(state);

    log::info!("Initialization complete. Click canvas to capture mouse.");
}

fn show_error(document: &web_sys::Document, message: &str) {
    if let Some(error_el) = document.get_element_by_id("error") {
        error_el.set_text_content(Some(message));
        let _ = error_el
            .dyn_ref::<web_sys::HtmlElement>()
            .map(|el| el.style().set_property("display", "block"));
    }
}

fn setup_resize_handler(canvas: &HtmlCanvasElement, state: Rc<RefCell<AppState>>) {
    let canvas_clone = canvas.clone();
    let closure = Closure::wrap(Box::new(move || {
        let Some(window) = web_sys::window() else { return };
        let logical_width = window
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(800.0);
        let logical_height = window
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(600.0);
        let dpr = window.device_pixel_ratio();
        let physical_width = (logical_width * dpr).round() as u32;
        let physical_height = (logical_height * dpr).round() as u32;

        canvas_clone.set_width(physical_width);
        canvas_clone.set_height(physical_height);

        if let Some(element) = canvas_clone.dyn_ref::<web_sys::HtmlElement>() {
            let _ = element.style().set_property("width", &format!("{logical_width}px"));
            let _ = element
                .style()
                .set_property("height", &format!("{logical_height}px"));
        }

        state.borrow_mut().resize(physical_width, physical_height);
    }) as Box<dyn FnMut()>);

    if let Some(window) = web_sys::window() {
        let _ = window.add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

fn start_frame_loop(state: Rc<RefCell<AppState>>) {
    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
        {
            let mut state = state.borrow_mut();
            // update() returns dt, which render() needs for particle simulation
            let dt = state.update(timestamp);
            state.render(dt);
        }

        // Request next frame
        if let Some(window) = web_sys::window() {
            if let Some(closure) = f.borrow().as_ref() {
                let _ = window.request_animation_frame(closure.as_ref().unchecked_ref());
            }
        }
    }));

    if let Some(window) = web_sys::window() {
        if let Some(closure) = g.borrow().as_ref() {
            let _ = window.request_animation_frame(closure.as_ref().unchecked_ref());
        }
    }
}

fn default_settings_to_js<T, F>(map_fn: F, label: &str) -> Result<JsValue, JsValue>
where
    T: Serialize + Default,
    F: Fn(presets::FullPreset) -> T,
{
    let settings = presets::get_default_preset()
        .map(map_fn)
        .unwrap_or_default();
    serde_wasm_bindgen::to_value(&settings)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize {}: {}", label, e)))
}

/// Update terrain settings from JavaScript
/// Called with a JS object containing settings fields
#[wasm_bindgen]
pub fn update_terrain_settings(settings_js: JsValue) -> Result<(), JsValue> {
    let settings: TerrainSettings = serde_wasm_bindgen::from_value(settings_js)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse settings: {}", e)))?;
    with_app_state_mut(|state| state.update_terrain_settings(settings))
}

/// Get current terrain settings as a JS object
#[wasm_bindgen]
pub fn get_terrain_settings() -> Result<JsValue, JsValue> {
    let settings = with_app_state(|state| state.get_terrain_settings().clone())?;
    serde_wasm_bindgen::to_value(&settings)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize settings: {}", e)))
}

/// Regenerate terrain with current settings (called from JS on R key press)
#[wasm_bindgen]
pub fn regenerate_terrain() -> Result<(), JsValue> {
    with_app_state_mut(|state| state.queue_terrain_regeneration())
}

/// Update sky settings from JavaScript
#[wasm_bindgen]
pub fn update_sky_settings(settings_js: JsValue) -> Result<(), JsValue> {
    let settings: SkySettings = serde_wasm_bindgen::from_value(settings_js)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse sky settings: {}", e)))?;
    with_app_state_mut(|state| state.update_sky_settings(settings))
}

/// Get current sky settings as a JS object
#[wasm_bindgen]
pub fn get_sky_settings() -> Result<JsValue, JsValue> {
    let settings = with_app_state(|state| state.get_sky_settings().clone())?;
    serde_wasm_bindgen::to_value(&settings)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize sky settings: {}", e)))
}

/// Update particle settings from JavaScript
#[wasm_bindgen]
pub fn update_particle_settings(settings_js: JsValue) -> Result<(), JsValue> {
    let settings: ParticleSettings = serde_wasm_bindgen::from_value(settings_js)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse particle settings: {}", e)))?;
    with_app_state_mut(|state| state.update_particle_settings(settings))
}

/// Get current particle settings as a JS object
#[wasm_bindgen]
pub fn get_particle_settings() -> Result<JsValue, JsValue> {
    let settings = with_app_state(|state| state.get_particle_settings().clone())?;
    serde_wasm_bindgen::to_value(&settings)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize particle settings: {}", e)))
}

/// Get default terrain settings (before app initialization)
#[wasm_bindgen]
pub fn get_default_terrain_settings() -> Result<JsValue, JsValue> {
    default_settings_to_js(|p| p.terrain, "terrain defaults")
}

/// Get default sky settings (before app initialization)
#[wasm_bindgen]
pub fn get_default_sky_settings() -> Result<JsValue, JsValue> {
    default_settings_to_js(|p| p.sky, "sky defaults")
}

/// Get default particle settings (before app initialization)
#[wasm_bindgen]
pub fn get_default_particle_settings() -> Result<JsValue, JsValue> {
    default_settings_to_js(|p| p.particles, "particle defaults")
}

/// Get list of available presets (id, name)
#[wasm_bindgen]
pub fn get_preset_list() -> Result<JsValue, JsValue> {
    let list = presets::get_preset_list();
    serde_wasm_bindgen::to_value(&list)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize preset list: {}", e)))
}

/// Get a full preset by ID
#[wasm_bindgen]
pub fn get_preset(id: &str) -> Result<JsValue, JsValue> {
    match presets::get_preset(id) {
        Some(preset) => serde_wasm_bindgen::to_value(&preset)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize preset: {}", e))),
        None => Err(JsValue::from_str(&format!("Unknown preset: {}", id))),
    }
}

/// Get the default preset ID
#[wasm_bindgen]
pub fn get_default_preset_id() -> String {
    presets::get_default_preset_id().to_string()
}
