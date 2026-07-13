use std::env;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::embedding::NeuralProfile;

#[derive(Debug, Clone, Serialize)]
pub struct HardwareReport {
    pub platform: String,
    pub cpu_threads: usize,
    pub nvidia_gpu: Option<String>,
    pub nvidia_compute_capability: Option<String>,
    pub installed_build: String,
    pub recommended_build: String,
    pub model_profile: String,
    pub accelerator_applies_to_profile: bool,
    pub recommended_runtime_ready: bool,
    pub optimal_build: bool,
    pub missing_libraries: Vec<String>,
    pub limitation: Option<String>,
    pub remediation: Option<String>,
    pub install_command: Option<String>,
    pub note: String,
}

pub fn inspect() -> HardwareReport {
    let platform = format!("{}-{}", env::consts::OS, env::consts::ARCH);
    let cpu_threads = std::thread::available_parallelism().map_or(1, usize::from);
    let (nvidia_gpu, nvidia_compute_capability) = nvidia_info();
    let installed_build = compiled_build().to_string();
    let mut missing_libraries = Vec::new();

    let cuda_compatible = nvidia_compute_capability
        .as_deref()
        .is_some_and(cuda_13_supports);
    let recommended_build = recommended_build(
        env::consts::OS,
        env::consts::ARCH,
        nvidia_gpu.is_some() && cuda_compatible,
    );
    let limitation = (env::consts::OS == "linux"
        && env::consts::ARCH == "x86_64"
        && nvidia_gpu.is_some()
        && !cuda_compatible)
        .then(|| {
            "Shipped CUDA 13 build requires NVIDIA compute capability 7.5 or newer.".to_string()
        });
    if recommended_build == "cuda" {
        for library in ["libcuda.so.1", "libcublas.so.13", "libcurand.so.10"] {
            if !library_available(library) {
                missing_libraries.push(library.to_string());
            }
        }
    }
    let recommended_build = recommended_build.to_string();
    let profile = NeuralProfile::configured();
    let accelerator_applies_to_profile = matches!(
        profile,
        NeuralProfile::General | NeuralProfile::Code | NeuralProfile::CodeHighQuality
    );

    let recommended_runtime_ready = missing_libraries.is_empty();
    let optimal_build = installed_build == recommended_build && recommended_runtime_ready;
    let remediation = (!missing_libraries.is_empty()).then(|| {
        format!(
            "Install NVIDIA driver and CUDA 13 runtime libraries providing: {}. Then run ig hardware again.",
            missing_libraries.join(", ")
        )
    });
    let install_command = (recommended_runtime_ready && installed_build != recommended_build).then(|| {
        format!(
            "curl -fsSL https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.sh | IVYGREP_ACCELERATOR={} sh",
            recommended_build
        )
    });
    let note = if recommended_build == "portable" {
        "Portable build uses optimized CPU inference and all available CPU threads.".to_string()
    } else {
        "CUDA/Metal accelerates transformer profiles; the default static profile remains CPU-optimized."
            .to_string()
    };

    HardwareReport {
        platform,
        cpu_threads,
        nvidia_gpu,
        nvidia_compute_capability,
        installed_build,
        recommended_build,
        model_profile: profile.name().to_string(),
        accelerator_applies_to_profile,
        recommended_runtime_ready,
        optimal_build,
        missing_libraries,
        limitation,
        remediation,
        install_command,
        note,
    }
}

fn recommended_build(os: &str, arch: &str, has_nvidia_gpu: bool) -> &'static str {
    match (os, arch, has_nvidia_gpu) {
        ("macos", "aarch64", _) => "metal",
        ("linux", "x86_64", true) => "cuda",
        _ => "portable",
    }
}

fn cuda_13_supports(capability: &str) -> bool {
    let mut parts = capability.trim().split('.');
    let Some(major) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
        return false;
    };
    let minor = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    major > 7 || (major == 7 && minor >= 5)
}

fn compiled_build() -> &'static str {
    if cfg!(feature = "cuda") {
        "cuda"
    } else if cfg!(all(feature = "metal", target_os = "macos")) {
        "metal"
    } else {
        "portable"
    }
}

fn nvidia_info() -> (Option<String>, Option<String>) {
    let Ok(output) = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,compute_cap",
            "--format=csv,noheader,nounits",
        ])
        .output()
    else {
        return (None, None);
    };
    if !output.status.success() {
        return (None, None);
    }
    parse_nvidia_info(&String::from_utf8_lossy(&output.stdout))
}

fn parse_nvidia_info(output: &str) -> (Option<String>, Option<String>) {
    let Some(line) = output.lines().next() else {
        return (None, None);
    };
    let mut fields = line.split(',').map(str::trim);
    (
        fields
            .next()
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        fields
            .next()
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    )
}

fn library_available(name: &str) -> bool {
    if let Ok(path) = env::var("IVYGREP_CUDA_LIBRARY_PATH")
        && env::split_paths(&path).any(|directory| directory.join(name).exists())
    {
        return true;
    }
    if Command::new("ldconfig")
        .arg("-p")
        .output()
        .is_ok_and(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains(name)
        })
    {
        return true;
    }
    [
        "/usr/local/cuda/lib64",
        "/usr/local/cuda/targets/x86_64-linux/lib",
        "/usr/lib/x86_64-linux-gnu",
        "/lib/x86_64-linux-gnu",
    ]
    .iter()
    .any(|directory| Path::new(directory).join(name).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_build_has_one_stable_label() {
        assert!(matches!(compiled_build(), "portable" | "cuda" | "metal"));
    }

    #[test]
    fn report_always_includes_actionable_cpu_information() {
        let report = inspect();
        assert!(!report.platform.is_empty());
        assert!(report.cpu_threads > 0);
        assert!(matches!(
            report.recommended_build.as_str(),
            "portable" | "cuda" | "metal"
        ));
        assert!(!report.model_profile.is_empty());
    }

    #[test]
    fn recommendations_match_supported_accelerator_archives() {
        assert_eq!(recommended_build("linux", "x86_64", true), "cuda");
        assert_eq!(recommended_build("linux", "x86_64", false), "portable");
        assert_eq!(recommended_build("linux", "aarch64", true), "portable");
        assert_eq!(recommended_build("macos", "aarch64", false), "metal");
        assert_eq!(recommended_build("macos", "x86_64", false), "portable");
        assert_eq!(recommended_build("windows", "x86_64", true), "portable");
    }

    #[test]
    fn cuda_13_requires_turing_or_newer() {
        assert!(!cuda_13_supports("5.2"));
        assert!(!cuda_13_supports("6.1"));
        assert!(!cuda_13_supports("7.0"));
        assert!(cuda_13_supports("7.5"));
        assert!(cuda_13_supports("8.6"));
        assert!(cuda_13_supports("12.0"));
        assert!(!cuda_13_supports("unknown"));
    }

    #[test]
    fn parses_first_nvidia_gpu() {
        assert_eq!(
            parse_nvidia_info("NVIDIA GeForce RTX 3060, 8.6\nNVIDIA T4, 7.5\n"),
            (
                Some("NVIDIA GeForce RTX 3060".to_string()),
                Some("8.6".to_string())
            )
        );
        assert_eq!(parse_nvidia_info(""), (None, None));
    }
}
