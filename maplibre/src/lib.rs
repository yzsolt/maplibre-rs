//! # Maplibre-rs
//!
//! A multi-platform library for rendering vector tile maps with WebGPU.
//!
//! Maplibre-rs is a map renderer that can run natively on MacOS, Linux, Windows, Android, iOS and the web.
//! It takes advantage of Lyon to tessellate vector tiles and WebGPU to display them efficiently.
//! Maplibre-rs also has an headless mode (*work in progress*) that can generate rasters.
//!
//! The official guide book can be found [here](https://maxammann.org/maplibre-rs/docs/).
//!
//! ### Example
//!
//! To import maplibre-rs in your `Cargo.toml`:
//!
//! ```toml
//! maplibre = "0.0.2"
//! ```

use crate::io::scheduler::{ScheduleMethod, Scheduler};
use crate::io::source_client::HTTPClient;
use crate::map_schedule::MapSchedule;
use crate::render::settings::{RendererSettings, WgpuSettings};
use crate::render::{RenderState, Renderer};
use crate::style::Style;
use crate::window::{MapWindow, MapWindowConfig, Runnable, WindowSize};

pub mod context;
pub mod coords;
pub mod error;
pub mod io;
// Exposed because of input handlers in maplibre-winit
pub mod map_schedule;
pub mod platform;
// Exposed because of camera
pub mod render;
pub mod style;
pub mod window;
// Exposed because of doc-strings
pub mod schedule;

// Used for benchmarking
pub mod benchmarking;

// Internal modules
pub(crate) mod stages;
pub(crate) mod tessellation;
pub(crate) mod util;

/// Map's configuration and execution.
pub struct Map<W, SM, HC>
where
    W: MapWindow,
    SM: ScheduleMethod,
    HC: HTTPClient,
{
    map_state: MapSchedule<W::MapWindowConfig, SM, HC>,
    window: W,
}

impl<W, SM, HC> Map<W, SM, HC>
where
    W: MapWindow + Runnable<W::MapWindowConfig, SM, HC>,
    SM: ScheduleMethod,
    HC: HTTPClient,
{
    /// Starts the [`crate::map_state::MapState`] Runnable with the configured event loop.
    pub fn run(self) {
        self.run_with_optionally_max_frames(None);
    }

    /// Starts the [`crate::map_state::MapState`] Runnable with the configured event loop.
    ///
    /// # Arguments
    ///
    /// * `max_frames` - Maximum number of frames per second.
    pub fn run_with_max_frames(self, max_frames: u64) {
        self.run_with_optionally_max_frames(Some(max_frames));
    }

    /// Starts the MapState Runnable with the configured event loop.
    ///
    /// # Arguments
    ///
    /// * `max_frames` - Optional maximum number of frames per second.
    pub fn run_with_optionally_max_frames(self, max_frames: Option<u64>) {
        self.window.run(self.map_state, max_frames);
    }
}

/// Stores the map configuration before the map's state has been fully initialized.
///
/// FIXME: We could maybe remove this class, and store the render_state in an Optional in [`crate::map_state::MapState`].
/// FIXME: I think we can find a workaround so that this class doesn't exist.
pub struct UninitializedMap<MWC, SM, HC>
where
    MWC: MapWindowConfig,
    SM: ScheduleMethod,
    HC: HTTPClient,
{
    scheduler: Scheduler<SM>,
    http_client: HC,
    style: Style,

    wgpu_settings: WgpuSettings,
    renderer_settings: RendererSettings,
    map_window_config: MWC,
}

impl<MWC, SM, HC> UninitializedMap<MWC, SM, HC>
where
    MWC: MapWindowConfig,
    SM: ScheduleMethod,
    HC: HTTPClient,
{
    /// Initializes the whole rendering pipeline for the given configuration.
    /// Returns the initialized map, ready to be run.
    pub async fn initialize(self) -> Map<MWC::MapWindow, SM, HC> {
        let window = MWC::MapWindow::create(&self.map_window_config);
        let window_size = window.size();

        #[cfg(target_os = "android")]
        let renderer = None;
        #[cfg(not(target_os = "android"))]
        let renderer = Renderer::initialize(
            &window,
            self.wgpu_settings.clone(),
            self.renderer_settings.clone(),
        )
        .await
        .ok();
        Map {
            map_state: MapSchedule::new(
                self.map_window_config,
                window_size,
                renderer,
                self.scheduler,
                self.http_client,
                self.style,
                self.wgpu_settings,
                self.renderer_settings,
            ),
            window,
        }
    }
}

pub struct MapBuilder<MWC, SM, HC>
where
    SM: ScheduleMethod,
{
    schedule_method: Option<SM>,
    scheduler: Option<Scheduler<SM>>,
    http_client: Option<HC>,
    style: Option<Style>,

    map_window_config: Option<MWC>,
    wgpu_settings: Option<WgpuSettings>,
    renderer_settings: Option<RendererSettings>,
}

impl<MWC, SM, HC> MapBuilder<MWC, SM, HC>
where
    MWC: MapWindowConfig,
    SM: ScheduleMethod,
    HC: HTTPClient,
{
    pub fn new() -> Self {
        Self {
            schedule_method: None,
            scheduler: None,
            http_client: None,
            style: None,
            map_window_config: None,
            wgpu_settings: None,
            renderer_settings: None,
        }
    }

    pub fn with_map_window_config(mut self, map_window_config: MWC) -> Self {
        self.map_window_config = Some(map_window_config);
        self
    }

    pub fn with_renderer_settings(mut self, renderer_settings: RendererSettings) -> Self {
        self.renderer_settings = Some(renderer_settings);
        self
    }

    pub fn with_wgpu_settings(mut self, wgpu_settings: WgpuSettings) -> Self {
        self.wgpu_settings = Some(wgpu_settings);
        self
    }

    pub fn with_schedule_method(mut self, schedule_method: SM) -> Self {
        self.schedule_method = Some(schedule_method);
        self
    }

    pub fn with_http_client(mut self, http_client: HC) -> Self {
        self.http_client = Some(http_client);
        self
    }

    pub fn with_existing_scheduler(mut self, scheduler: Scheduler<SM>) -> Self {
        self.scheduler = Some(scheduler);
        self
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Builds the UninitializedMap with the given configuration.
    pub fn build(self) -> UninitializedMap<MWC, SM, HC> {
        let scheduler = self
            .scheduler
            .unwrap_or_else(|| Scheduler::new(self.schedule_method.unwrap()));
        let style = self.style.unwrap_or_default();

        UninitializedMap {
            scheduler,
            http_client: self.http_client.unwrap(),
            style,
            wgpu_settings: self.wgpu_settings.unwrap_or_default(),
            renderer_settings: self.renderer_settings.unwrap_or_default(),
            map_window_config: self.map_window_config.unwrap(),
        }
    }
}
