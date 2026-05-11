//! Hardware probes for onboarding: detects CPU/RAM/GPU and local Ollama
//! availability, then maps those facts to a ranked list of embedding model
//! options. Pure probing — no tool surface.

/// System facts gathered at onboarding time for model selection.
#[derive(Debug, serde::Serialize)]
pub struct HardwareContext {
    pub ollama_available: bool,
    pub ollama_host: String,
    pub gpu: Option<GpuInfo>,
    pub ram_gb: u64,
    pub cpu_cores: u32,
}

/// GPU vendor and VRAM info (best-effort; None means no GPU detected).
#[derive(Debug, serde::Serialize)]
#[serde(tag = "vendor", rename_all = "lowercase")]
pub enum GpuInfo {
    Nvidia { name: String, vram_mb: u64 },
    Amd { name: String, vram_mb: Option<u64> },
}

/// One entry in the ranked model recommendation list.
#[derive(Debug, serde::Serialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub dims: u32,
    pub context_tokens: u32,
    pub reason: String,
    pub available: bool,
    pub recommended: bool,
}

/// Pure function: derive a ranked model list from hardware facts.
/// The first entry is always the recommended default (Ollama URL hint when
/// available, otherwise the external-server URL hint).
pub fn model_options_for_hardware(ctx: &HardwareContext) -> Vec<ModelOption> {
    let mut options: Vec<ModelOption> = Vec::new();

    if ctx.ollama_available {
        options.push(ModelOption {
            id: "url".into(),
            label: "Use running Ollama".into(),
            dims: 768,
            context_tokens: 8192,
            reason: format!(
                "set url = \"{}/v1\" in project.toml to use your running Ollama",
                ctx.ollama_host.trim_end_matches('/')
            ),
            available: true,
            recommended: true,
        });
    } else {
        options.push(ModelOption {
            id: "url".into(),
            label: "External server".into(),
            dims: 0,
            context_tokens: 0,
            reason: "set url in [embeddings] to use any OpenAI-compatible embedding server (e.g. Ollama, llama-server, TEI, vLLM)".into(),
            available: true,
            recommended: true,
        });
    }

    options
}

/// Extract a `host:port` string suitable for `TcpStream::connect` from an
/// Ollama host URL like `http://localhost:11434`.
pub(crate) fn ollama_tcp_addr(host: &str) -> String {
    let stripped = host
        .strip_prefix("https://")
        .or_else(|| host.strip_prefix("http://"))
        .unwrap_or(host);
    if stripped.contains(':') {
        stripped.to_string()
    } else {
        format!("{stripped}:11434")
    }
}

/// Returns true if a TCP connection to Ollama's port succeeds within 2s.
async fn probe_ollama(tcp_addr: &str) -> bool {
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::net::TcpStream::connect(tcp_addr),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

/// Probe NVIDIA GPU via nvidia-smi. Returns None if not available.
async fn probe_nvidia() -> Option<GpuInfo> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total",
                "--format=csv,noheader,nounits",
            ])
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?;
    let mut parts = line.splitn(2, ',');
    let name = parts.next()?.trim().to_string();
    let vram_mb: u64 = parts.next()?.trim().parse().ok()?;
    Some(GpuInfo::Nvidia { name, vram_mb })
}

/// Probe AMD GPU via rocm-smi. Returns None if not available.
async fn probe_amd() -> Option<GpuInfo> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::process::Command::new("rocm-smi")
            .arg("--showproductname")
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // rocm-smi output contains lines like "Card series:  AMD Radeon RX 7900 XTX"
    let name = stdout
        .lines()
        .find(|l| {
            let l = l.to_lowercase();
            l.contains("card series") || l.contains("card model") || l.contains("radeon")
        })
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_else(|| "AMD GPU".into());
    Some(GpuInfo::Amd {
        name,
        vram_mb: None,
    })
}

/// Read total system RAM in GiB. Returns 0 on failure (non-fatal).
async fn probe_ram() -> u64 {
    // Linux: /proc/meminfo — use spawn_blocking to avoid blocking the async executor.
    #[cfg(target_os = "linux")]
    {
        let meminfo = tokio::task::spawn_blocking(|| std::fs::read_to_string("/proc/meminfo"))
            .await
            .ok()
            .and_then(|r| r.ok());
        if let Some(content) = meminfo {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    let kb: u64 = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    return kb / 1024 / 1024;
                }
            }
        }
    }
    // macOS: sysctl hw.memsize. Gated so we don't spawn sysctl on Linux
    // when /proc/meminfo parse already failed.
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = tokio::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .await
        {
            if let Ok(s) = String::from_utf8(output.stdout) {
                if let Ok(bytes) = s.trim().parse::<u64>() {
                    return bytes / 1024 / 1024 / 1024;
                }
            }
        }
    }
    0
}

/// Probe the local system for hardware capabilities relevant to embedding
/// model selection. All probes run in parallel with a 2-second timeout;
/// any failure produces a safe zero/None default — never panics.
pub async fn detect_hardware_context() -> HardwareContext {
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
    let tcp_addr = ollama_tcp_addr(&ollama_host);

    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);

    let (ollama_available, nvidia, amd, ram_gb) = tokio::join!(
        probe_ollama(&tcp_addr),
        probe_nvidia(),
        probe_amd(),
        probe_ram(),
    );

    // NVIDIA wins if both somehow respond (shouldn't happen, but be defensive)
    let gpu = nvidia.or(amd);

    HardwareContext {
        ollama_available,
        ollama_host,
        gpu,
        ram_gb,
        cpu_cores,
    }
}

#[cfg(test)]
mod tests {
    use super::{model_options_for_hardware, ollama_tcp_addr, GpuInfo, HardwareContext};

    #[test]
    fn model_options_ollama_available_recommends_allminilm() {
        let ctx = HardwareContext {
            ollama_available: true,
            ollama_host: "http://localhost:11434".into(),
            gpu: None,
            ram_gb: 16,
            cpu_cores: 8,
        };
        let opts = model_options_for_hardware(&ctx);
        // With Ollama: a single recommended url hint pointing at the running Ollama.
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].id, "url");
        assert!(opts[0].recommended);
        assert!(opts[0].reason.contains("Ollama"));
    }

    #[test]
    fn model_options_cpu_only_recommends_jina() {
        let ctx = HardwareContext {
            ollama_available: false,
            ollama_host: "http://localhost:11434".into(),
            gpu: None,
            ram_gb: 8,
            cpu_cores: 4,
        };
        let opts = model_options_for_hardware(&ctx);
        // Without Ollama: a single recommended url hint for an external server.
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].id, "url");
        assert!(opts[0].recommended);
    }

    #[test]
    fn model_options_exactly_one_recommended() {
        let ctx = HardwareContext {
            ollama_available: true,
            ollama_host: "http://localhost:11434".into(),
            gpu: Some(GpuInfo::Nvidia {
                name: "RTX 3080".into(),
                vram_mb: 10240,
            }),
            ram_gb: 32,
            cpu_cores: 16,
        };
        let opts = model_options_for_hardware(&ctx);
        let recommended_count = opts.iter().filter(|o| o.recommended).count();
        assert_eq!(recommended_count, 1);
    }

    #[test]
    fn model_options_default_is_url_when_no_ollama() {
        let hw = HardwareContext {
            ollama_available: false,
            ollama_host: "http://localhost:11434".into(),
            gpu: None,
            ram_gb: 16,
            cpu_cores: 8,
        };
        let options = model_options_for_hardware(&hw);
        assert_eq!(options[0].id, "url");
        assert!(options[0].recommended);
        // Must mention url in the reason
        assert!(
            options.iter().any(|o| o.reason.contains("url")),
            "must mention url as an option"
        );
    }

    #[test]
    fn model_options_with_ollama_recommends_url() {
        let hw = HardwareContext {
            ollama_available: true,
            ollama_host: "http://localhost:11434".into(),
            gpu: None,
            ram_gb: 16,
            cpu_cores: 8,
        };
        let options = model_options_for_hardware(&hw);
        assert_eq!(options[0].id, "url");
        assert!(options[0].recommended);
        // Ollama option should mention url
        assert!(
            options
                .iter()
                .any(|o| o.reason.contains("url") || o.reason.contains("Ollama")),
            "must mention Ollama or url option"
        );
    }

    #[test]
    fn ollama_tcp_addr_strips_http_prefix() {
        assert_eq!(ollama_tcp_addr("http://localhost:11434"), "localhost:11434");
        assert_eq!(ollama_tcp_addr("https://remote:11434"), "remote:11434");
        assert_eq!(ollama_tcp_addr("localhost:11434"), "localhost:11434");
        assert_eq!(ollama_tcp_addr("myhost"), "myhost:11434");
    }
}
