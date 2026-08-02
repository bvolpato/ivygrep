use std::process::Command;
use std::time::Duration;

use anyhow::Result;

use crate::embedding::EmbeddingModel;
use crate::system_resources::available_memory_bytes;

const MIB: u64 = 1024 * 1024;
const NEURAL_CPU_BATCH_SIZE: usize = 64;
const NEURAL_STATIC_BATCH_SIZE: usize = 1024;
const NEURAL_CUDA_BATCH_SIZE: usize = 8;
const NEURAL_METAL_BATCH_SIZE: usize = 256;
const NEURAL_CUDA_HIGH_PRESSURE_FREE_BYTES: u64 = 2 * 1024 * MIB;
const NEURAL_CUDA_MEDIUM_PRESSURE_FREE_BYTES: u64 = 4 * 1024 * MIB;
const NEURAL_CUDA_SHARED_FREE_BYTES: u64 = 8 * 1024 * MIB;
const NEURAL_CUDA_HIGH_PRESSURE_FREE_PERCENT: u64 = 20;
const NEURAL_CUDA_MEDIUM_PRESSURE_FREE_PERCENT: u64 = 35;
const NEURAL_CUDA_SHARED_FREE_PERCENT: u64 = 50;
const NEURAL_CUDA_HIGH_UTILIZATION_PERCENT: u32 = 70;
const NEURAL_CUDA_BUSY_UTILIZATION_PERCENT: u32 = 35;
const NEURAL_CUDA_ACTIVE_UTILIZATION_PERCENT: u32 = 25;
const MAX_CONFIGURED_NEURAL_BATCH_SIZE: usize = 4096;

pub(super) const NEURAL_BATCH_SIZE_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

pub(super) fn indexing_pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(indexing_worker_count())
            .thread_name(|index| format!("ivygrep-index-{index}"))
            .build()
            .expect("indexing thread pool must build")
    })
}

fn indexing_worker_count() -> usize {
    let logical = num_cpus::get().max(1);
    configured_indexing_worker_count(
        logical,
        num_cpus::get_physical(),
        std::env::var("IVYGREP_INDEX_THREADS").ok().as_deref(),
    )
}

fn configured_indexing_worker_count(
    logical: usize,
    physical: usize,
    configured: Option<&str>,
) -> usize {
    configured
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or(physical)
        .clamp(1, logical)
}

pub(super) fn neural_enhance_batch_size(neural_model: &dyn EmbeddingModel) -> usize {
    let backend = neural_model.backend_info();
    let cuda_resources = backend
        .filter(|backend| backend.contains("Candle CUDA"))
        .and_then(|_| cuda_resource_snapshot());
    neural_enhance_batch_size_for(backend, cuda_resources, configured_neural_batch_size())
}

fn configured_neural_batch_size() -> Option<usize> {
    std::env::var("IVYGREP_NEURAL_BATCH_SIZE")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(MAX_CONFIGURED_NEURAL_BATCH_SIZE))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct CudaResourceSnapshot {
    free_bytes: u64,
    total_bytes: u64,
    utilization_percent: u32,
}

impl CudaResourceSnapshot {
    fn free_percent(self) -> u64 {
        if self.total_bytes == 0 {
            return 0;
        }
        ((self.free_bytes as u128 * 100) / self.total_bytes as u128) as u64
    }
}

fn parse_cuda_resource_snapshot(line: &str) -> Option<CudaResourceSnapshot> {
    let mut parts = line.split(',').map(str::trim);
    let free_mib = parts.next()?.parse::<u64>().ok()?;
    let total_mib = parts.next()?.parse::<u64>().ok()?;
    let utilization_percent = parts.next()?.parse::<u32>().ok()?;
    Some(CudaResourceSnapshot {
        free_bytes: free_mib.checked_mul(MIB)?,
        total_bytes: total_mib.checked_mul(MIB)?,
        utilization_percent,
    })
}

fn cuda_resource_snapshot() -> Option<CudaResourceSnapshot> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.free,memory.total,utilization.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout.lines().next().and_then(parse_cuda_resource_snapshot)
}

fn cuda_neural_enhance_batch_size(resources: Option<CudaResourceSnapshot>) -> usize {
    let Some(resources) = resources else {
        return NEURAL_CUDA_BATCH_SIZE;
    };
    let free_percent = resources.free_percent();
    if resources.utilization_percent >= NEURAL_CUDA_HIGH_UTILIZATION_PERCENT
        || resources.free_bytes < NEURAL_CUDA_HIGH_PRESSURE_FREE_BYTES
        || free_percent <= NEURAL_CUDA_HIGH_PRESSURE_FREE_PERCENT
    {
        return 1;
    }
    if resources.utilization_percent >= NEURAL_CUDA_BUSY_UTILIZATION_PERCENT
        || resources.free_bytes < NEURAL_CUDA_MEDIUM_PRESSURE_FREE_BYTES
        || free_percent <= NEURAL_CUDA_MEDIUM_PRESSURE_FREE_PERCENT
    {
        return (NEURAL_CUDA_BATCH_SIZE / 4).max(1);
    }
    if resources.utilization_percent >= NEURAL_CUDA_ACTIVE_UTILIZATION_PERCENT
        || resources.free_bytes < NEURAL_CUDA_SHARED_FREE_BYTES
        || free_percent <= NEURAL_CUDA_SHARED_FREE_PERCENT
    {
        return (NEURAL_CUDA_BATCH_SIZE / 2).max(1);
    }
    NEURAL_CUDA_BATCH_SIZE
}

fn neural_enhance_batch_size_for(
    backend: Option<&str>,
    cuda_resources: Option<CudaResourceSnapshot>,
    configured: Option<usize>,
) -> usize {
    if let Some(configured) = configured {
        return configured;
    }

    match backend {
        Some(backend) if backend.contains("StaticEmbedding") || backend.contains("Model2Vec") => {
            NEURAL_STATIC_BATCH_SIZE
        }
        Some(backend) if backend.contains("Candle CUDA") => {
            cuda_neural_enhance_batch_size(cuda_resources)
        }
        Some(backend) if backend.contains("Candle Metal") => NEURAL_METAL_BATCH_SIZE,
        _ => NEURAL_CPU_BATCH_SIZE,
    }
}

/// Refuse foreground indexing when available memory is dangerously low.
pub(super) fn check_memory_before_index() -> Result<()> {
    if cfg!(test) || std::env::var("CI").is_ok() {
        return Ok(());
    }

    if let Some(bytes) = available_memory_bytes()
        && bytes < 512 * MIB
    {
        anyhow::bail!(
            "refusing to index: only {} MiB of memory available (need at least 512 MiB). \
             Close other applications or free memory before re-indexing.",
            bytes / MIB
        );
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_pmset_batt(stdout: &str) -> Option<String> {
    stdout
        .contains("Battery Power")
        .then(|| "Battery Power".to_string())
}

#[cfg(target_os = "macos")]
fn parse_pmset_therm(stdout: &str) -> Option<String> {
    (stdout.contains("warning level")
        && !stdout.contains("No thermal warning level")
        && !stdout.contains("No performance warning level"))
    .then(|| "Thermal Throttling".to_string())
}

/// Load-average multiple of CPU count that pauses background neural work.
/// `IVYGREP_ENHANCE_MAX_LOAD_RATIO` overrides the 2.0 default; values at or
/// below zero disable the check.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn enhance_max_load_ratio() -> f64 {
    std::env::var("IVYGREP_ENHANCE_MAX_LOAD_RATIO")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(2.0)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn parse_system_load(load1: f64, cpus: f64) -> Option<String> {
    let ratio = enhance_max_load_ratio();
    if ratio <= 0.0 {
        return None;
    }
    let max_load = cpus * ratio;
    (load1 > max_load).then(|| format!("High System Load ({load1:.1} > {max_load:.1} max)"))
}

#[cfg(target_os = "macos")]
pub(super) fn check_system_constraints() -> Option<String> {
    if cfg!(test) || std::env::var("CI").is_ok() {
        return None;
    }
    if let Some(reason) = low_available_memory_reason(1024 * MIB) {
        return Some(reason);
    }
    if let Ok(output) = Command::new("pmset").args(["-g", "batt"]).output()
        && let Some(reason) = parse_pmset_batt(&String::from_utf8_lossy(&output.stdout))
    {
        return Some(reason);
    }
    if let Ok(output) = Command::new("pmset").args(["-g", "therm"]).output()
        && let Some(reason) = parse_pmset_therm(&String::from_utf8_lossy(&output.stdout))
    {
        return Some(reason);
    }
    high_load_reason()
}

#[cfg(target_os = "linux")]
pub(super) fn check_system_constraints() -> Option<String> {
    if cfg!(test) || std::env::var("CI").is_ok() {
        return None;
    }
    high_load_reason().or_else(|| low_available_memory_reason(1024 * MIB))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn high_load_reason() -> Option<String> {
    let mut loadavg = [0.0f64; 3];
    let has_load = unsafe { libc::getloadavg(loadavg.as_mut_ptr(), 3) };
    (has_load > 0)
        .then(|| parse_system_load(loadavg[0], num_cpus::get() as f64))
        .flatten()
}

#[cfg(all(not(target_os = "macos"), not(target_os = "linux")))]
pub(super) fn check_system_constraints() -> Option<String> {
    low_available_memory_reason(1024 * MIB)
}

fn low_available_memory_reason(minimum_bytes: u64) -> Option<String> {
    available_memory_bytes()
        .filter(|available| *available < minimum_bytes)
        .map(|available| format!("Low Available Memory ({} MiB)", available / MIB))
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    fn indexing_workers_default_to_physical_cores_and_respect_bounds() {
        assert_eq!(configured_indexing_worker_count(32, 16, None), 16);
        assert_eq!(configured_indexing_worker_count(32, 16, Some("64")), 32);
        assert_eq!(configured_indexing_worker_count(32, 16, Some("8")), 8);
        assert_eq!(
            configured_indexing_worker_count(32, 16, Some("invalid")),
            16
        );
        assert_eq!(configured_indexing_worker_count(32, 16, Some("0")), 16);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    #[serial]
    fn enhance_load_throttle_is_lenient_and_configurable() {
        unsafe { std::env::remove_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO") };
        assert!(parse_system_load(8.0, 8.0).is_none());
        assert!(parse_system_load(20.0, 8.0).is_some());
        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0.5") };
        assert!(parse_system_load(8.0, 8.0).is_some());
        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0") };
        assert!(parse_system_load(100.0, 8.0).is_none());
        unsafe { std::env::remove_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO") };
    }

    #[test]
    fn neural_batch_size_scales_for_accelerators() {
        let cuda_resources =
            |free_gib: u64, total_gib: u64, utilization_percent: u32| CudaResourceSnapshot {
                free_bytes: free_gib * 1024 * MIB,
                total_bytes: total_gib * 1024 * MIB,
                utilization_percent,
            };

        assert_eq!(
            neural_enhance_batch_size_for(None, None, None),
            NEURAL_CPU_BATCH_SIZE
        );
        assert_eq!(
            neural_enhance_batch_size_for(Some("BERT embedding via Candle CPU"), None, None),
            NEURAL_CPU_BATCH_SIZE
        );
        assert_eq!(
            neural_enhance_batch_size_for(Some("StaticEmbedding token mean via Rust"), None, None),
            NEURAL_STATIC_BATCH_SIZE
        );
        assert_eq!(
            neural_enhance_batch_size_for(
                Some("Model2Vec weighted token mean via Rust"),
                None,
                None,
            ),
            NEURAL_STATIC_BATCH_SIZE
        );
        assert_eq!(
            neural_enhance_batch_size_for(
                Some("BERT embedding via Candle CUDA"),
                Some(cuda_resources(14, 16, 0)),
                None,
            ),
            NEURAL_CUDA_BATCH_SIZE
        );
        assert_eq!(
            neural_enhance_batch_size_for(
                Some("BERT embedding via Candle CUDA"),
                Some(cuda_resources(6, 16, 0)),
                None,
            ),
            NEURAL_CUDA_BATCH_SIZE / 2
        );
        assert_eq!(
            neural_enhance_batch_size_for(
                Some("BERT embedding via Candle CUDA"),
                Some(cuda_resources(5, 16, 0)),
                None,
            ),
            NEURAL_CUDA_BATCH_SIZE / 4
        );
        assert_eq!(
            neural_enhance_batch_size_for(
                Some("BERT embedding via Candle CUDA"),
                Some(cuda_resources(14, 16, 75)),
                None,
            ),
            1
        );
        assert_eq!(
            neural_enhance_batch_size_for(
                Some("BERT embedding via Candle CUDA"),
                Some(cuda_resources(1, 16, 0)),
                None,
            ),
            1
        );
        assert_eq!(
            neural_enhance_batch_size_for(Some("BERT embedding via Candle CUDA"), None, None),
            NEURAL_CUDA_BATCH_SIZE
        );
        assert_eq!(
            neural_enhance_batch_size_for(
                Some("BERT embedding via Candle CUDA"),
                Some(cuda_resources(1, 16, 90)),
                Some(32),
            ),
            32
        );
        assert_eq!(
            neural_enhance_batch_size_for(Some("BERT embedding via Candle Metal"), None, None),
            NEURAL_METAL_BATCH_SIZE
        );
    }

    #[test]
    fn parses_cuda_resource_snapshot_from_nvidia_smi() {
        let snapshot = parse_cuda_resource_snapshot("13988, 16303, 7").unwrap();
        assert_eq!(snapshot.free_bytes, 13_988 * MIB);
        assert_eq!(snapshot.total_bytes, 16_303 * MIB);
        assert_eq!(snapshot.utilization_percent, 7);
        assert_eq!(snapshot.free_percent(), 85);
    }

    #[test]
    #[serial]
    fn neural_batch_size_env_override_is_bounded() {
        unsafe { std::env::set_var("IVYGREP_NEURAL_BATCH_SIZE", "128") };
        assert_eq!(configured_neural_batch_size(), Some(128));
        unsafe { std::env::set_var("IVYGREP_NEURAL_BATCH_SIZE", "999999") };
        assert_eq!(
            configured_neural_batch_size(),
            Some(MAX_CONFIGURED_NEURAL_BATCH_SIZE)
        );
        unsafe { std::env::set_var("IVYGREP_NEURAL_BATCH_SIZE", "0") };
        assert_eq!(configured_neural_batch_size(), None);
        unsafe { std::env::remove_var("IVYGREP_NEURAL_BATCH_SIZE") };
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn parses_pmset_battery_state() {
        let ac = "Now drawing from 'AC Power'";
        let battery = "Now drawing from 'Battery Power'";
        assert_eq!(parse_pmset_batt(ac), None);
        assert_eq!(parse_pmset_batt(battery), Some("Battery Power".to_string()));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn parses_pmset_thermal_state() {
        let normal = "Note: No thermal warning level has been recorded";
        let throttled = "Note: Thermal warning level CPU_Speed_Limit = 50";
        assert_eq!(parse_pmset_therm(normal), None);
        assert_eq!(
            parse_pmset_therm(throttled),
            Some("Thermal Throttling".to_string())
        );
    }
}
