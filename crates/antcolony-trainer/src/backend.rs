//! Backend abstraction — wraps the underlying tensor framework so we can
//! swap Candle for Aether (or anything else) without rewriting PPO logic.
//!
//! The trait surface is intentionally minimal: just what PPO actually
//! needs. Adding ops here = adding to `J:\aether\ANTCOLONY_FR.md`.

use candle_core::{Device, Tensor};

/// Backend trait. Methods correspond 1:1 to Aether FR list items.
pub trait Backend {
    fn device(&self) -> &Device;
    fn cuda_available(&self) -> bool;
}

/// Candle backend — current production choice. CUDA-accelerated when
/// the `cuda` feature is enabled at compile time.
pub struct CandleBackend {
    device: Device,
}

impl CandleBackend {
    /// Create a new Candle backend, preferring CUDA on RTX 3070 Ti
    /// (compute_cap=86, Ampere) when available, falling back to CPU.
    pub fn new() -> anyhow::Result<Self> {
        // Try CUDA first; fall back to CPU with a tracing warning.
        let device = if cfg!(feature = "cuda") {
            match Device::new_cuda(0) {
                Ok(d) => {
                    tracing::info!("CandleBackend: CUDA device 0 (RTX 3070 Ti expected)");
                    d
                }
                Err(e) => {
                    tracing::warn!(error = %e, "CUDA init failed, falling back to CPU");
                    Device::Cpu
                }
            }
        } else {
            tracing::info!("CandleBackend: CPU (build without --features cuda for GPU)");
            Device::Cpu
        };
        Ok(Self { device })
    }

    pub fn cpu() -> Self {
        Self { device: Device::Cpu }
    }

    pub fn tensor(&self) -> &Device { &self.device }
}

impl Backend for CandleBackend {
    fn device(&self) -> &Device { &self.device }
    fn cuda_available(&self) -> bool { matches!(self.device, Device::Cuda(_)) }
}

/// Helper: build a 17-element input tensor from a ColonyAiState.
pub fn state_to_tensor(state: &antcolony_sim::ColonyAiState, device: &Device) -> anyhow::Result<Tensor> {
    let ed = if state.enemy_distance_min.is_finite() { state.enemy_distance_min } else { 1e6 };
    let raw: Vec<f32> = vec![
        state.food_stored, state.food_inflow_recent,
        state.worker_count as f32, state.soldier_count as f32, state.breeder_count as f32,
        state.brood_egg as f32, state.brood_larva as f32, state.brood_pupa as f32,
        state.queens_alive as f32, state.combat_losses_recent as f32,
        ed, state.enemy_worker_count as f32, state.enemy_soldier_count as f32,
        state.day_of_year as f32, state.ambient_temp_c,
        if state.diapause_active { 1.0 } else { 0.0 },
        if state.is_daytime { 1.0 } else { 0.0 },
    ];
    // Unsqueeze to [1, INPUT_DIM] — Candle's Linear expects a batch dim.
    Ok(Tensor::from_vec(raw, (1, crate::INPUT_DIM), device)?)
}
