use std::process::Command;
#[cfg(any(target_os = "macos", test))]
use std::sync::Mutex;
use std::time::Duration;
#[cfg(any(target_os = "macos", test))]
use std::time::Instant;

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
const DEFAULT_TANTIVY_MEMORY_BUDGET: usize = 50_000_000;
const TANTIVY_MEMORY_PER_WORKER: usize = 16_000_000;
const LARGE_TANTIVY_MEMORY_PER_WORKER: usize = 24_000_000;
const SMALL_TANTIVY_WORKLOAD_BYTES: u64 = 512 * 1024;
const MEDIUM_TANTIVY_WORKLOAD_BYTES: u64 = 2 * MIB;
const LARGE_TANTIVY_WORKLOAD_BYTES: u64 = 8 * MIB;

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

pub(super) fn tantivy_writer_settings(indexed_source_bytes: u64) -> (usize, usize) {
    configured_tantivy_writer_settings(indexed_source_bytes, indexing_worker_count())
}

fn configured_tantivy_writer_settings(
    indexed_source_bytes: u64,
    indexing_workers: usize,
) -> (usize, usize) {
    let workload_workers = if indexed_source_bytes < SMALL_TANTIVY_WORKLOAD_BYTES {
        2
    } else if indexed_source_bytes < MEDIUM_TANTIVY_WORKLOAD_BYTES {
        4
    } else {
        6
    };
    let writer_threads = indexing_workers.clamp(1, workload_workers);
    let memory_per_worker = if indexed_source_bytes >= LARGE_TANTIVY_WORKLOAD_BYTES {
        LARGE_TANTIVY_MEMORY_PER_WORKER
    } else {
        TANTIVY_MEMORY_PER_WORKER
    };
    let memory_budget = DEFAULT_TANTIVY_MEMORY_BUDGET.max(writer_threads * memory_per_worker);
    (writer_threads, memory_budget)
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

/// Background vector enhancement tier asking whether it may run right now.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum EnhancementTier {
    /// Cheap static/hash embeddings. Pauses only for memory and load since it is
    /// the only source of vector results before neural coverage exists.
    Hash,
    /// Transformer embeddings. Additionally pauses on battery and thermal
    /// pressure unless `IVYGREP_ENHANCE_ON_BATTERY=1` opts out of the battery pause.
    Neural,
}

/// Point-in-time view of host pressure signals feeding the pause decision.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct ConstraintSnapshot {
    low_memory: Option<String>,
    power: Option<PowerProbe>,
    high_load: Option<String>,
}

/// Result of the macOS `pmset` probes. Cached for `POWER_PROBE_TTL` so a batch
/// that takes ~25 ms does not fork `pmset` twice on every iteration.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
struct PowerProbe {
    on_battery: bool,
    thermal_throttled: bool,
}

#[cfg(target_os = "macos")]
const POWER_PROBE_TTL: Duration = Duration::from_secs(5);
const ENHANCE_MIN_AVAILABLE_MEMORY: u64 = 1024 * MIB;

fn enhancement_pause_reason(
    tier: EnhancementTier,
    snapshot: &ConstraintSnapshot,
    enhance_on_battery: bool,
) -> Option<String> {
    if let Some(reason) = &snapshot.low_memory {
        return Some(reason.clone());
    }
    if tier == EnhancementTier::Neural
        && let Some(power) = snapshot.power
    {
        if power.on_battery && !enhance_on_battery {
            return Some("Battery Power".to_string());
        }
        if power.thermal_throttled {
            return Some("Thermal Throttling".to_string());
        }
    }
    snapshot.high_load.clone()
}

/// Returns the cached value when it is younger than `ttl`, otherwise re-probes.
#[cfg(any(target_os = "macos", test))]
fn cached_probe<T: Copy>(
    cache: &Mutex<Option<(Instant, T)>>,
    ttl: Duration,
    now: Instant,
    probe: impl FnOnce() -> T,
) -> T {
    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((probed_at, value)) = *guard
        && now.saturating_duration_since(probed_at) < ttl
    {
        return value;
    }
    let value = probe();
    *guard = Some((now, value));
    value
}

#[cfg(target_os = "macos")]
fn parse_pmset_batt(stdout: &str) -> bool {
    stdout.contains("Battery Power")
}

#[cfg(target_os = "macos")]
fn parse_pmset_therm(stdout: &str) -> bool {
    stdout.contains("warning level")
        && !stdout.contains("No thermal warning level")
        && !stdout.contains("No performance warning level")
}

#[cfg(target_os = "macos")]
fn pmset_output(section: &str) -> Option<String> {
    Command::new("pmset")
        .args(["-g", section])
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "macos")]
fn probe_power() -> PowerProbe {
    PowerProbe {
        on_battery: pmset_output("batt").is_some_and(|stdout| parse_pmset_batt(&stdout)),
        thermal_throttled: pmset_output("therm").is_some_and(|stdout| parse_pmset_therm(&stdout)),
    }
}

#[cfg(target_os = "macos")]
fn cached_power_probe() -> PowerProbe {
    static CACHE: Mutex<Option<(Instant, PowerProbe)>> = Mutex::new(None);
    cached_probe(&CACHE, POWER_PROBE_TTL, Instant::now(), probe_power)
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

/// Reason the given enhancement tier should pause right now, if any.
pub(super) fn check_system_constraints(tier: EnhancementTier) -> Option<String> {
    if cfg!(test) || std::env::var("CI").is_ok() {
        return None;
    }
    let snapshot = constraint_snapshot(tier);
    enhancement_pause_reason(tier, &snapshot, crate::config::enhance_on_battery())
}

#[cfg(target_os = "macos")]
fn constraint_snapshot(tier: EnhancementTier) -> ConstraintSnapshot {
    ConstraintSnapshot {
        low_memory: low_available_memory_reason(ENHANCE_MIN_AVAILABLE_MEMORY),
        // Hash never consults power state, so skip the pmset fork entirely.
        power: (tier == EnhancementTier::Neural).then(cached_power_probe),
        high_load: high_load_reason(),
    }
}

#[cfg(target_os = "linux")]
fn constraint_snapshot(_tier: EnhancementTier) -> ConstraintSnapshot {
    ConstraintSnapshot {
        low_memory: low_available_memory_reason(ENHANCE_MIN_AVAILABLE_MEMORY),
        power: None,
        high_load: high_load_reason(),
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "linux")))]
fn constraint_snapshot(_tier: EnhancementTier) -> ConstraintSnapshot {
    ConstraintSnapshot {
        low_memory: low_available_memory_reason(ENHANCE_MIN_AVAILABLE_MEMORY),
        power: None,
        high_load: None,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn high_load_reason() -> Option<String> {
    let mut loadavg = [0.0f64; 3];
    let has_load = unsafe { libc::getloadavg(loadavg.as_mut_ptr(), 3) };
    (has_load > 0)
        .then(|| parse_system_load(loadavg[0], num_cpus::get() as f64))
        .flatten()
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

    #[test]
    fn tantivy_writer_scales_with_workload_without_exceeding_index_workers() {
        assert_eq!(configured_tantivy_writer_settings(0, 16), (2, 50_000_000));
        assert_eq!(
            configured_tantivy_writer_settings(SMALL_TANTIVY_WORKLOAD_BYTES, 16),
            (4, 64_000_000)
        );
        assert_eq!(
            configured_tantivy_writer_settings(MEDIUM_TANTIVY_WORKLOAD_BYTES, 16),
            (6, 96_000_000)
        );
        assert_eq!(
            configured_tantivy_writer_settings(LARGE_TANTIVY_WORKLOAD_BYTES, 16),
            (6, 144_000_000)
        );
        assert_eq!(
            configured_tantivy_writer_settings(LARGE_TANTIVY_WORKLOAD_BYTES, 4),
            (4, 96_000_000)
        );
        assert_eq!(
            configured_tantivy_writer_settings(LARGE_TANTIVY_WORKLOAD_BYTES, 1),
            (1, 50_000_000)
        );
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
        assert!(!parse_pmset_batt(ac));
        assert!(parse_pmset_batt(battery));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn parses_pmset_thermal_state() {
        let normal = "Note: No thermal warning level has been recorded";
        let throttled = "Note: Thermal warning level CPU_Speed_Limit = 50";
        assert!(!parse_pmset_therm(normal));
        assert!(parse_pmset_therm(throttled));
    }

    fn on_battery() -> ConstraintSnapshot {
        ConstraintSnapshot {
            power: Some(PowerProbe {
                on_battery: true,
                thermal_throttled: false,
            }),
            ..ConstraintSnapshot::default()
        }
    }

    #[test]
    fn hash_tier_ignores_battery_and_thermal_pressure() {
        let snapshot = ConstraintSnapshot {
            power: Some(PowerProbe {
                on_battery: true,
                thermal_throttled: true,
            }),
            ..ConstraintSnapshot::default()
        };
        assert_eq!(
            enhancement_pause_reason(EnhancementTier::Hash, &snapshot, false),
            None
        );
    }

    #[test]
    fn hash_tier_still_pauses_for_memory_and_load() {
        let low_memory = ConstraintSnapshot {
            low_memory: Some("Low Available Memory (512 MiB)".to_string()),
            ..on_battery()
        };
        assert_eq!(
            enhancement_pause_reason(EnhancementTier::Hash, &low_memory, false).as_deref(),
            Some("Low Available Memory (512 MiB)")
        );
        let high_load = ConstraintSnapshot {
            high_load: Some("High System Load (20.0 > 16.0 max)".to_string()),
            ..on_battery()
        };
        assert_eq!(
            enhancement_pause_reason(EnhancementTier::Hash, &high_load, false).as_deref(),
            Some("High System Load (20.0 > 16.0 max)")
        );
    }

    #[test]
    fn neural_tier_pauses_on_battery_unless_opted_out() {
        let snapshot = on_battery();
        assert_eq!(
            enhancement_pause_reason(EnhancementTier::Neural, &snapshot, false).as_deref(),
            Some("Battery Power")
        );
        assert_eq!(
            enhancement_pause_reason(EnhancementTier::Neural, &snapshot, true),
            None
        );
    }

    #[test]
    fn neural_tier_battery_opt_out_keeps_thermal_memory_and_load_pauses() {
        let throttled = ConstraintSnapshot {
            power: Some(PowerProbe {
                on_battery: true,
                thermal_throttled: true,
            }),
            ..ConstraintSnapshot::default()
        };
        assert_eq!(
            enhancement_pause_reason(EnhancementTier::Neural, &throttled, true).as_deref(),
            Some("Thermal Throttling")
        );
        let low_memory = ConstraintSnapshot {
            low_memory: Some("Low Available Memory (256 MiB)".to_string()),
            ..on_battery()
        };
        assert_eq!(
            enhancement_pause_reason(EnhancementTier::Neural, &low_memory, true).as_deref(),
            Some("Low Available Memory (256 MiB)")
        );
        let high_load = ConstraintSnapshot {
            high_load: Some("High System Load (40.0 > 16.0 max)".to_string()),
            ..on_battery()
        };
        assert_eq!(
            enhancement_pause_reason(EnhancementTier::Neural, &high_load, true).as_deref(),
            Some("High System Load (40.0 > 16.0 max)")
        );
    }

    #[test]
    fn cached_probe_reuses_value_within_ttl_and_refreshes_after() {
        let cache: Mutex<Option<(Instant, u32)>> = Mutex::new(None);
        let ttl = Duration::from_secs(5);
        let start = Instant::now();
        let mut calls = 0u32;
        let mut probe = || {
            calls += 1;
            calls
        };

        assert_eq!(cached_probe(&cache, ttl, start, &mut probe), 1);
        assert_eq!(
            cached_probe(&cache, ttl, start + Duration::from_secs(4), &mut probe),
            1
        );
        assert_eq!(
            cached_probe(&cache, ttl, start + Duration::from_secs(5), &mut probe),
            2
        );
        assert_eq!(
            cached_probe(&cache, ttl, start + Duration::from_secs(6), &mut probe),
            2
        );
        assert_eq!(calls, 2);
    }
}
