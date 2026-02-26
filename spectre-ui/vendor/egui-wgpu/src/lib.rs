//! This crates provides bindings between [`egui`](https://github.com/emilk/egui) and [wgpu](https://crates.io/crates/wgpu).
//! Vendored with `force_fallback_adapter` support for software (WARP) rendering.

#![allow(unsafe_code)]

pub use wgpu;

mod renderer;

pub use renderer::*;

#[cfg(feature = "winit")]
pub mod winit;

use std::sync::Arc;

use epaint::mutex::RwLock;

/// An error produced by egui-wgpu.
#[derive(thiserror::Error, Debug)]
pub enum WgpuError {
    #[error("Failed to create wgpu adapter, no suitable adapter found.")]
    NoSuitableAdapterFound,

    #[error("There was no valid format for the surface at all.")]
    NoSurfaceFormatsAvailable,

    #[error(transparent)]
    RequestDeviceError(#[from] wgpu::RequestDeviceError),

    #[error(transparent)]
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),

    #[cfg(feature = "winit")]
    #[error(transparent)]
    HandleError(#[from] ::winit::raw_window_handle::HandleError),
}

/// Access to the render state for egui.
#[derive(Clone)]
pub struct RenderState {
    pub adapter: Arc<wgpu::Adapter>,

    #[cfg(not(target_arch = "wasm32"))]
    pub available_adapters: Arc<[wgpu::Adapter]>,

    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub target_format: wgpu::TextureFormat,
    pub renderer: Arc<RwLock<Renderer>>,
}

impl RenderState {
    pub async fn create(
        config: &WgpuConfiguration,
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'static>,
        depth_format: Option<wgpu::TextureFormat>,
        msaa_samples: u32,
    ) -> Result<Self, WgpuError> {
        crate::profile_scope!("RenderState::create");

        #[cfg(not(target_arch = "wasm32"))]
        let available_adapters = instance.enumerate_adapters(wgpu::Backends::all());

        let adapter = {
            crate::profile_scope!("request_adapter");
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: config.power_preference,
                    compatible_surface: Some(surface),
                    force_fallback_adapter: config.force_fallback_adapter,
                })
                .await
                .ok_or_else(|| {
                    #[cfg(not(target_arch = "wasm32"))]
                    if available_adapters.is_empty() {
                        log::info!("No wgpu adapters found");
                    } else if available_adapters.len() == 1 {
                        log::info!(
                            "The only available wgpu adapter was not suitable: {}",
                            adapter_info_summary(&available_adapters[0].get_info())
                        );
                    } else {
                        log::info!(
                            "No suitable wgpu adapter found out of the {} available ones: {}",
                            available_adapters.len(),
                            describe_adapters(&available_adapters)
                        );
                    }

                    WgpuError::NoSuitableAdapterFound
                })?
        };

        #[cfg(target_arch = "wasm32")]
        log::debug!(
            "Picked wgpu adapter: {}",
            adapter_info_summary(&adapter.get_info())
        );

        #[cfg(not(target_arch = "wasm32"))]
        if available_adapters.len() == 1 {
            log::debug!(
                "Picked the only available wgpu adapter: {}",
                adapter_info_summary(&adapter.get_info())
            );
        } else {
            log::info!(
                "There were {} available wgpu adapters: {}",
                available_adapters.len(),
                describe_adapters(&available_adapters)
            );
            log::debug!(
                "Picked wgpu adapter: {}",
                adapter_info_summary(&adapter.get_info())
            );
        }

        let capabilities = {
            crate::profile_scope!("get_capabilities");
            surface.get_capabilities(&adapter).formats
        };
        let target_format = crate::preferred_framebuffer_format(&capabilities)?;

        let (device, queue) = {
            crate::profile_scope!("request_device");
            adapter
                .request_device(&(*config.device_descriptor)(&adapter), None)
                .await?
        };

        let renderer = Renderer::new(&device, target_format, depth_format, msaa_samples);

        Ok(Self {
            adapter: Arc::new(adapter),
            #[cfg(not(target_arch = "wasm32"))]
            available_adapters: available_adapters.into(),
            device: Arc::new(device),
            queue: Arc::new(queue),
            target_format,
            renderer: Arc::new(RwLock::new(renderer)),
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn describe_adapters(adapters: &[wgpu::Adapter]) -> String {
    if adapters.is_empty() {
        "(none)".to_owned()
    } else if adapters.len() == 1 {
        adapter_info_summary(&adapters[0].get_info())
    } else {
        let mut list_string = String::new();
        for adapter in adapters {
            if !list_string.is_empty() {
                list_string += ", ";
            }
            list_string += &format!("{{{}}}", adapter_info_summary(&adapter.get_info()));
        }
        list_string
    }
}

pub enum SurfaceErrorAction {
    SkipFrame,
    RecreateSurface,
}

/// Configuration for using wgpu with eframe or the egui-wgpu winit feature.
#[derive(Clone)]
pub struct WgpuConfiguration {
    pub supported_backends: wgpu::Backends,
    pub device_descriptor: Arc<dyn Fn(&wgpu::Adapter) -> wgpu::DeviceDescriptor<'static>>,
    pub present_mode: wgpu::PresentMode,
    pub desired_maximum_frame_latency: Option<u32>,
    pub power_preference: wgpu::PowerPreference,
    /// When true, only a software/fallback adapter (e.g. WARP on Windows) is used. For headless/server or no-GPU environments.
    pub force_fallback_adapter: bool,
    pub on_surface_error: Arc<dyn Fn(wgpu::SurfaceError) -> SurfaceErrorAction>,
}

impl std::fmt::Debug for WgpuConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuConfiguration")
            .field("supported_backends", &self.supported_backends)
            .field("present_mode", &self.present_mode)
            .field("desired_maximum_frame_latency", &self.desired_maximum_frame_latency)
            .field("power_preference", &self.power_preference)
            .field("force_fallback_adapter", &self.force_fallback_adapter)
            .finish_non_exhaustive()
    }
}

impl Default for WgpuConfiguration {
    fn default() -> Self {
        Self {
            supported_backends: wgpu::util::backend_bits_from_env()
                .unwrap_or(wgpu::Backends::PRIMARY | wgpu::Backends::GL),

            device_descriptor: Arc::new(|adapter| {
                let base_limits = if adapter.get_info().backend == wgpu::Backend::Gl {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                };

                wgpu::DeviceDescriptor {
                    label: Some("egui wgpu device"),
                    required_features: wgpu::Features::default(),
                    required_limits: wgpu::Limits {
                        max_texture_dimension_2d: 8192,
                        ..base_limits
                    },
                }
            }),

            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: None,

            power_preference: wgpu::util::power_preference_from_env()
                .unwrap_or(wgpu::PowerPreference::HighPerformance),

            force_fallback_adapter: false,

            on_surface_error: Arc::new(|err| {
                if err == wgpu::SurfaceError::Outdated {
                } else {
                    log::warn!("Dropped frame with error: {err}");
                }
                SurfaceErrorAction::SkipFrame
            }),
        }
    }
}

pub fn preferred_framebuffer_format(
    formats: &[wgpu::TextureFormat],
) -> Result<wgpu::TextureFormat, WgpuError> {
    for &format in formats {
        if matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
        ) {
            return Ok(format);
        }
    }

    formats
        .first()
        .copied()
        .ok_or(WgpuError::NoSurfaceFormatsAvailable)
}

pub fn depth_format_from_bits(depth_buffer: u8, stencil_buffer: u8) -> Option<wgpu::TextureFormat> {
    match (depth_buffer, stencil_buffer) {
        (0, 8) => Some(wgpu::TextureFormat::Stencil8),
        (16, 0) => Some(wgpu::TextureFormat::Depth16Unorm),
        (24, 0) => Some(wgpu::TextureFormat::Depth24Plus),
        (24, 8) => Some(wgpu::TextureFormat::Depth24PlusStencil8),
        (32, 0) => Some(wgpu::TextureFormat::Depth32Float),
        (32, 8) => Some(wgpu::TextureFormat::Depth32FloatStencil8),
        _ => None,
    }
}

pub fn adapter_info_summary(info: &wgpu::AdapterInfo) -> String {
    let wgpu::AdapterInfo {
        name,
        vendor,
        device,
        device_type,
        driver,
        driver_info,
        backend,
    } = info;

    let mut summary = format!("backend: {backend:?}, device_type: {device_type:?}");

    if !name.is_empty() {
        summary += &format!(", name: {name:?}");
    }
    if !driver.is_empty() {
        summary += &format!(", driver: {driver:?}");
    }
    if !driver_info.is_empty() {
        summary += &format!(", driver_info: {driver_info:?}");
    }
    if *vendor != 0 {
        summary += &format!(", vendor: 0x{vendor:04X}");
    }
    if *device != 0 {
        summary += &format!(", device: 0x{device:02X}");
    }

    summary
}

mod profiling_scopes {
    #![allow(unused_macros)]
    #![allow(unused_imports)]

    macro_rules! profile_function {
        ($($arg: tt)*) => {
            #[cfg(feature = "puffin")]
            #[cfg(not(target_arch = "wasm32"))]
            puffin::profile_function!($($arg)*);
        };
    }
    pub(crate) use profile_function;

    macro_rules! profile_scope {
        ($($arg: tt)*) => {
            #[cfg(feature = "puffin")]
            #[cfg(not(target_arch = "wasm32"))]
            puffin::profile_scope!($($arg)*);
        };
    }
    pub(crate) use profile_scope;
}

#[allow(unused_imports)]
pub(crate) use profiling_scopes::*;
