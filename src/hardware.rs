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

impl GpuInfo {
    /// Short human label for recommendation text, e.g. `RTX 3080 (10240 MB VRAM)`.
    pub fn describe(&self) -> String {
        match self {
            GpuInfo::Nvidia { name, vram_mb } => format!("{name} ({vram_mb} MB VRAM)"),
            GpuInfo::Amd {
                name,
                vram_mb: Some(mb),
            } => format!("{name} ({mb} MB VRAM)"),
            GpuInfo::Amd {
                name,
                vram_mb: None,
            } => name.clone(),
        }
    }
}

/// What a [`ModelOption`] tells the caller to write into `[embeddings]`.
///
/// Until 2026-09-17 both server options carried `id: "url"` — a token that is
/// not a model name, sitting in the one field onboarding copies verbatim into
/// `[embeddings].model`. That was harmless only because a `local:` entry was
/// hardcoded first, so a server entry could never rank first. Feature-aware
/// ranking removes that accident: on a build without a local backend a server
/// entry *does* lead, and the untyped form would have written `model = "url"`,
/// failing at index time with `Unknown model 'url'`. The variant makes that
/// unrepresentable instead of merely unlikely.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum OptionTarget {
    /// Write `model` to `[embeddings].model`; its prefix selects the backend.
    Model { model: String },
    /// Write `url` to `[embeddings].url` and `model` to `[embeddings].model`,
    /// where it becomes the name sent on the wire. A `None` url means the
    /// operator has to supply one — nothing here can guess it.
    Server {
        url: Option<String>,
        model: Option<String>,
    },
}

impl OptionTarget {
    /// The value for `[embeddings].model`, if this option names one.
    pub fn model(&self) -> Option<&str> {
        match self {
            OptionTarget::Model { model } => Some(model),
            OptionTarget::Server { model, .. } => model.as_deref(),
        }
    }

    /// The value for `[embeddings].url`, if this option names one.
    pub fn url(&self) -> Option<&str> {
        match self {
            OptionTarget::Model { .. } => None,
            OptionTarget::Server { url, .. } => url.as_deref(),
        }
    }
}

/// Which embedding backends this binary can actually construct.
///
/// Carried as DATA rather than read from `cfg!` inside the ranking, so the
/// ranking stays pure and both directions are reachable from one lane. A
/// `cfg!` in the ranking body would make the no-local case unreachable in the
/// default lane and the local case unreachable in the lean one — whichever
/// lane ran would exercise half the branches and report a full pass, which is
/// the same shape as the vacuity CLAUDE.md § Testing Discipline records for
/// the lean and `server-stack` lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompiledBackends {
    /// `local:` / `local-dir:` models can be loaded in-process.
    pub local: bool,
    /// `url`, `ollama:` and `openai:` targets can be reached over HTTP.
    pub remote: bool,
}

impl CompiledBackends {
    /// What THIS binary was built with — the single place `cfg!` is read.
    ///
    /// Mirrors the feature test in `src/retrieval/client.rs`, which asks the
    /// same question about the same two features when classifying a backend.
    pub fn current() -> Self {
        Self {
            local: cfg!(any(
                feature = "local-embed",
                feature = "local-embed-dynamic"
            )),
            remote: cfg!(feature = "remote-embed"),
        }
    }
}

/// One entry in the ranked model recommendation list.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelOption {
    /// What to write into `[embeddings]` to take this option. Replaces the
    /// former `id: String`, which conflated a model name with the sentinel
    /// `"url"` — see [`OptionTarget`].
    pub target: OptionTarget,
    pub label: String,
    pub dims: u32,
    pub context_tokens: u32,
    pub reason: String,
    /// True when this option works as-is on this binary and this host: the
    /// backend is compiled in and any server it names is already reachable.
    /// False means the operator has to do something first (rebuild, start a
    /// server, supply a url or an API key).
    pub available: bool,
    pub recommended: bool,
}

impl ModelOption {
    /// The `[embeddings]` section that taking this option produces.
    ///
    /// Extracted from `perform_full_onboarding` so the mapping is reachable
    /// without running the tool: onboarding probes real hardware, so the
    /// `Server` branch is unreachable from a test there on any host that has a
    /// local backend compiled — the branch that writes a url would have shipped
    /// covered only by whichever machine happened to run it.
    pub fn embeddings_section(&self) -> crate::config::project::EmbeddingsSection {
        crate::config::project::EmbeddingsSection {
            model: self.target.model().map(str::to_string),
            url: self.target.url().map(str::to_string),
            ..Default::default()
        }
    }
}

/// A code-specialized 768d model earns its ~300MB download and its CPU cost
/// only on a host that can absorb both; below either bound the 22MB/384d model
/// leads. **Load-bearing fixture values:** `model_options_rank_differs_across_hosts`
/// straddles these two numbers, so moving either without moving that test's
/// fixtures leaves it comparing two hosts on the same side of the boundary —
/// still green, no longer a differential.
const JINA_MIN_RAM_GB: u64 = 16;
const JINA_MIN_CORES: u32 = 8;

/// Pure function: derive a ranked model list from hardware facts.
///
/// Reads the binary's own compiled feature set via [`CompiledBackends::current`];
/// [`model_options_for`] takes it as an argument for tests.
pub fn model_options_for_hardware(ctx: &HardwareContext) -> Vec<ModelOption> {
    model_options_for(ctx, CompiledBackends::current())
}

/// The ranking proper — pure in both its hardware facts and its backend set.
///
/// Ranking rules, and what each hardware fact is actually evidence for:
///
/// - `gpu` ranks the **Ollama** entry and never a `local:` one. The local ONNX
///   path is CPU-only in every shipped configuration: `codescout-embed`
///   selects `ort`'s CPU prebuilt (`ort-download-binaries-native-tls`) or the
///   dynamic C ABI, and `local.rs` registers no execution provider. A GPU
///   therefore accelerates what Ollama serves and does nothing for `local:`.
///   Ranking a local model on GPU presence would be confidently wrong.
/// - `ram_gb` and `cpu_cores` choose between the two local models. Local
///   embedding is CPU-bound (`spawn_blocking` around fastembed) and runs over
///   the whole repo at index time, so a 300MB code model on a small host is a
///   real cost rather than a preference.
/// - `ollama_available` decides whether the Ollama entry exists at all.
pub(crate) fn model_options_for(
    ctx: &HardwareContext,
    backends: CompiledBackends,
) -> Vec<ModelOption> {
    let mut options: Vec<ModelOption> = Vec::new();
    let ollama_usable = ctx.ollama_available && backends.remote;

    let ollama_entry = |recommended: bool| ModelOption {
        target: OptionTarget::Server {
            url: Some(format!("{}/v1", ctx.ollama_host.trim_end_matches('/'))),
            model: Some("nomic-embed-text".into()),
        },
        label: "Ollama (running)".into(),
        dims: 768,
        context_tokens: 8192,
        reason: match &ctx.gpu {
            Some(gpu) => format!(
                "{} detected with Ollama already up — it serves embeddings on the GPU, \
                 while local: models are CPU-only in this build",
                gpu.describe()
            ),
            None => {
                "Ollama is reachable — 768d/8192-token embeddings with no model download".into()
            }
        },
        available: true,
        recommended,
    };

    let jina_entry = |recommended: bool| ModelOption {
        target: OptionTarget::Model {
            model: "local:JinaEmbeddingsV2BaseCode".into(),
        },
        label: "JinaEmbeddingsV2BaseCode".into(),
        dims: 768,
        context_tokens: 8192,
        reason: format!(
            "code-specialized ONNX, 8192-token context; ~300MB download and CPU-bound \
             at index time, which {} GB RAM / {} cores can absorb",
            ctx.ram_gb, ctx.cpu_cores
        ),
        available: true,
        recommended,
    };

    let minilm_entry = |recommended: bool| ModelOption {
        target: OptionTarget::Model {
            model: crate::config::project::default_embed_model(),
        },
        label: "AllMiniLML6V2Q".into(),
        dims: 384,
        context_tokens: 256,
        reason: format!(
            "bundled ONNX, no server needed, 22MB quantized — the light option on \
             {} GB RAM / {} cores",
            ctx.ram_gb, ctx.cpu_cores
        ),
        available: true,
        recommended,
    };

    // A GPU that is already serving beats any CPU-bound local model.
    let ollama_leads = ollama_usable && ctx.gpu.is_some();
    if ollama_leads {
        options.push(ollama_entry(true));
    }

    if backends.local {
        let roomy = ctx.ram_gb >= JINA_MIN_RAM_GB && ctx.cpu_cores >= JINA_MIN_CORES;
        let lead = options.is_empty();
        if roomy {
            options.push(jina_entry(lead));
            options.push(minilm_entry(false));
        } else {
            options.push(minilm_entry(lead));
            options.push(jina_entry(false));
        }
    }

    // Not `!options.iter().any(|o| o.label == "Ollama (running)")`: keying the
    // de-dup on a display string means renaming the label silently offers the
    // entry twice, and nothing would red — `exactly_one_recommended` still
    // holds, since the duplicate is not the recommended one.
    if ollama_usable && !ollama_leads {
        options.push(ollama_entry(options.is_empty()));
    }

    if backends.remote {
        // Captured BEFORE the first push: `options.is_empty()` read at the
        // external-server entry would already see the OpenAI entry and answer
        // false, leaving a remote-only build with nothing recommended at all.
        let nothing_leads_yet = options.is_empty();
        // The API-key path, surfaced as an option rather than left to the docs.
        options.push(ModelOption {
            target: OptionTarget::Model {
                model: "openai:text-embedding-3-small".into(),
            },
            label: "OpenAI API".into(),
            dims: 1536,
            context_tokens: 8191,
            reason: "hosted, no local compute — needs a key in [embeddings].api_key or \
                     $CODESCOUT_EMBEDDING_API_KEY; your code is sent to OpenAI"
                .into(),
            // Never "available": a key is required and we do not probe for one.
            available: false,
            recommended: false,
        });
        options.push(ModelOption {
            target: OptionTarget::Server {
                url: None,
                model: Some("nomic-embed-text".into()),
            },
            label: "External server".into(),
            dims: 0,
            context_tokens: 0,
            reason: "set [embeddings].url to any OpenAI-compatible /v1 endpoint \
                     (llama.cpp, vLLM, TEI); model is sent as the model name"
                .into(),
            available: false,
            recommended: nothing_leads_yet,
        });
    }

    // A build with neither backend still has to answer: onboarding calls
    // `.first().expect(...)`, so an empty vec would turn `--no-default-features`
    // onboarding into a panic. Name the missing backend instead, and still write
    // the built-in default so a later rebuild finds a usable config.
    if options.is_empty() {
        options.push(ModelOption {
            target: OptionTarget::Model {
                model: crate::config::project::default_embed_model(),
            },
            label: "No embedding backend compiled".into(),
            dims: 384,
            context_tokens: 256,
            reason: "this binary was built without local-embed and without remote-embed — \
                     semantic search stays unavailable until it is rebuilt with one"
                .into(),
            available: false,
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

/// Whether to run the subprocess GPU probes (`nvidia-smi` / `rocm-smi`).
///
/// On Windows — especially locked-down VDIs — spawning these is taxed by EDR
/// injection, and `CreateProcessW` is synchronous: a hung spawn blocks the tokio
/// worker and the 2s `timeout` cannot preempt it. So the subprocess probe is
/// skipped on Windows by default; set `CODESCOUT_GPU_PROBE=1` to opt back in on a
/// real Windows GPU host. Non-Windows always probes. Pure for Linux-CI testing.
fn gpu_probe_enabled(is_windows: bool, opt_in: bool) -> bool {
    !is_windows || opt_in
}

/// Probe NVIDIA GPU via nvidia-smi. Returns None if not available.
async fn probe_nvidia() -> Option<GpuInfo> {
    if !gpu_probe_enabled(
        cfg!(windows),
        std::env::var_os("CODESCOUT_GPU_PROBE").is_some(),
    ) {
        return None;
    }
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
    if !gpu_probe_enabled(
        cfg!(windows),
        std::env::var_os("CODESCOUT_GPU_PROBE").is_some(),
    ) {
        return None;
    }
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
    use super::{
        model_options_for, model_options_for_hardware, ollama_tcp_addr, CompiledBackends, GpuInfo,
        HardwareContext, OptionTarget,
    };

    /// Both backends compiled — the shipped `default` feature set.
    const BOTH: CompiledBackends = CompiledBackends {
        local: true,
        remote: true,
    };
    /// `--no-default-features`: neither backend exists in the binary.
    const NEITHER: CompiledBackends = CompiledBackends {
        local: false,
        remote: false,
    };
    /// A lean-but-networked build, and the configuration `scripts/build-windows.sh`
    /// shipped before `local-embed-dynamic` became its default.
    const REMOTE_ONLY: CompiledBackends = CompiledBackends {
        local: false,
        remote: true,
    };

    fn host(ollama: bool, gpu: Option<GpuInfo>, ram_gb: u64, cpu_cores: u32) -> HardwareContext {
        HardwareContext {
            ollama_available: ollama,
            ollama_host: "http://localhost:11434".into(),
            gpu,
            ram_gb,
            cpu_cores,
        }
    }

    fn rtx3080() -> GpuInfo {
        GpuInfo::Nvidia {
            name: "RTX 3080".into(),
            vram_mb: 10240,
        }
    }

    /// **The differential.** Every per-case assertion in this module is monotone
    /// under deleting the ranking — a function that ignores hardware and returns
    /// one constant satisfies all of them, which is what the previous five tests
    /// did while varying `gpu`, `ram_gb` and `cpu_cores` across their fixtures.
    /// This one cannot be: it asserts two hosts DISAGREE, so a constant ranking
    /// makes both sides equal and reds it.
    #[test]
    fn model_options_rank_differs_across_hosts() {
        let small = model_options_for(&host(false, None, 8, 4), BOTH);
        let large = model_options_for(&host(false, None, 32, 16), BOTH);
        assert_ne!(
            small[0].target, large[0].target,
            "an 8GB/4-core host and a 32GB/16-core host must not get the same \
             recommendation — that equality is exactly the defect this replaced"
        );
    }

    /// The GPU half of the same differential, on the axis a GPU actually moves.
    /// Local ONNX is CPU-only, so a GPU may only re-rank the Ollama entry —
    /// which is why this pair holds `ollama_available` fixed and varies the GPU.
    #[test]
    fn a_gpu_promotes_ollama_over_a_cpu_bound_local_model() {
        let no_gpu = model_options_for(&host(true, None, 32, 16), BOTH);
        let with_gpu = model_options_for(&host(true, Some(rtx3080()), 32, 16), BOTH);
        assert_ne!(
            no_gpu[0].target, with_gpu[0].target,
            "a detected GPU must change what leads when Ollama is already running"
        );
        assert_eq!(with_gpu[0].label, "Ollama (running)");
        assert!(
            with_gpu[0].reason.contains("RTX 3080"),
            "the reason must name the GPU it ranked on, got: {}",
            with_gpu[0].reason
        );
    }

    /// A GPU with no Ollama to serve it must change nothing: no backend in that
    /// configuration can use it, and claiming otherwise would be the same class
    /// of defect pointing the other way.
    #[test]
    fn a_gpu_without_ollama_changes_nothing() {
        let plain = model_options_for(&host(false, None, 32, 16), BOTH);
        let gpu = model_options_for(&host(false, Some(rtx3080()), 32, 16), BOTH);
        assert_eq!(plain[0].target, gpu[0].target);
    }

    #[test]
    fn a_roomy_host_leads_with_the_code_model() {
        let opts = model_options_for(&host(false, None, 32, 16), BOTH);
        assert_eq!(
            opts[0].target,
            OptionTarget::Model {
                model: "local:JinaEmbeddingsV2BaseCode".into()
            }
        );
        assert!(opts[0].recommended);
    }

    /// Renamed 2026-09-17 from `model_options_cpu_only_recommends_jina`, whose
    /// body asserted AllMiniLM. The name was a fossil of a ranking that had
    /// already been replaced by a hardcoded constant, and it read as coverage
    /// for a Jina recommendation nothing produced.
    #[test]
    fn a_small_host_leads_with_the_light_model() {
        let opts = model_options_for(&host(false, None, 8, 4), BOTH);
        assert_eq!(
            opts[0].target,
            OptionTarget::Model {
                model: "local:AllMiniLML6V2Q".into()
            }
        );
        assert!(opts[0].recommended);
    }

    /// Both bounds are load-bearing. Each case clears one and fails the other,
    /// so a ranking that dropped either conjunct would lead with Jina here.
    #[test]
    fn either_bound_alone_is_not_enough_for_the_code_model() {
        for (ram, cores) in [(32u64, 4u32), (8, 16)] {
            let opts = model_options_for(&host(false, None, ram, cores), BOTH);
            assert_eq!(
                opts[0].target,
                OptionTarget::Model {
                    model: "local:AllMiniLML6V2Q".into()
                },
                "{ram}GB/{cores} cores clears only one bound and must not lead with Jina"
            );
        }
    }

    #[test]
    fn model_options_exactly_one_recommended() {
        for backends in [BOTH, NEITHER, REMOTE_ONLY] {
            for ctx in [
                host(true, Some(rtx3080()), 32, 16),
                host(false, None, 8, 4),
                host(true, None, 16, 8),
            ] {
                let opts = model_options_for(&ctx, backends);
                assert_eq!(
                    opts.iter().filter(|o| o.recommended).count(),
                    1,
                    "exactly one entry must be recommended for {backends:?}"
                );
            }
        }
    }

    /// `onboarding.rs` calls `.first().expect(...)`, so an empty list is a panic
    /// at onboarding time rather than a missing recommendation. The lean lane
    /// compiles neither backend, so this combination really ships.
    #[test]
    fn every_backend_combination_yields_at_least_one_option() {
        for local in [true, false] {
            for remote in [true, false] {
                let backends = CompiledBackends { local, remote };
                for ollama in [true, false] {
                    let opts = model_options_for(&host(ollama, None, 16, 8), backends);
                    assert!(
                        !opts.is_empty(),
                        "no options for {backends:?} with ollama={ollama}"
                    );
                }
            }
        }
    }

    /// A binary without a local backend answers `Local embedding requires the
    /// 'local-embed' feature` when it tries to load a `local:` model, so
    /// recommending one writes a config that cannot work.
    #[test]
    fn a_build_without_a_local_backend_never_recommends_a_local_model() {
        for ollama in [true, false] {
            for gpu in [None, Some(rtx3080())] {
                let opts = model_options_for(&host(ollama, gpu, 32, 16), REMOTE_ONLY);
                assert!(
                    opts.iter()
                        .all(|o| !o.target.model().is_some_and(|m| m.starts_with("local:"))),
                    "offered a local: model without a local backend: {:?}",
                    opts.iter().map(|o| &o.target).collect::<Vec<_>>()
                );
            }
        }
    }

    /// The typed-target guarantee, asserted where onboarding consumes it: the
    /// value destined for `[embeddings].model` is always a model string. The
    /// former shape put `id: "url"` on two entries, which onboarding would have
    /// copied verbatim the moment one of them led.
    #[test]
    fn no_option_offers_a_non_model_as_the_model() {
        for backends in [BOTH, NEITHER, REMOTE_ONLY] {
            for ollama in [true, false] {
                for o in model_options_for(&host(ollama, None, 16, 8), backends) {
                    if let Some(model) = o.target.model() {
                        assert_ne!(model, "url", "{}: `url` is not a model name", o.label);
                        assert!(
                            !model.is_empty(),
                            "{}: an empty model string would blank the config",
                            o.label
                        );
                    }
                }
            }
        }
    }

    /// No option may be offered twice. The Ollama entry is constructed at two
    /// call sites (lead position when a GPU serves it, trailing otherwise) and
    /// only one may fire. Nothing else reds on a duplicate:
    /// `model_options_exactly_one_recommended` still holds, because the
    /// duplicate is not the recommended one.
    #[test]
    fn no_option_is_offered_twice() {
        for backends in [BOTH, NEITHER, REMOTE_ONLY] {
            for ollama in [true, false] {
                for gpu in [None, Some(rtx3080())] {
                    let opts = model_options_for(&host(ollama, gpu, 32, 16), backends);
                    let mut seen: Vec<&OptionTarget> = Vec::new();
                    for o in &opts {
                        assert!(
                            !seen.contains(&&o.target),
                            "{:?} offered twice for {backends:?} ollama={ollama}",
                            o.target
                        );
                        seen.push(&o.target);
                    }
                }
            }
        }
    }

    /// The option → `[embeddings]` mapping onboarding writes, both variants.
    ///
    /// The `Server` branch is why this test exists: onboarding probes the real
    /// host, so on any machine with a local backend the recommended entry is a
    /// `Model` and the url-writing branch never executes. Asserting it through
    /// the tool would have made coverage a property of the test machine.
    #[test]
    fn taking_an_option_writes_the_fields_that_option_names() {
        let model_entry = model_options_for(&host(false, None, 8, 4), BOTH)
            .into_iter()
            .next()
            .expect("at least one option");
        let section = model_entry.embeddings_section();
        assert_eq!(section.model.as_deref(), Some("local:AllMiniLML6V2Q"));
        assert_eq!(
            section.url, None,
            "a local model must not also pin a url — that combination is refused \
             at construction time for local-dir: and is meaningless for local:"
        );

        let server_entry = model_options_for(&host(true, Some(rtx3080()), 32, 16), BOTH)
            .into_iter()
            .next()
            .expect("at least one option");
        let section = server_entry.embeddings_section();
        assert_eq!(section.url.as_deref(), Some("http://localhost:11434/v1"));
        assert_eq!(
            section.model.as_deref(),
            Some("nomic-embed-text"),
            "a server option must still name the model to send on the wire"
        );
    }

    /// The API-key path, surfaced as an option rather than left to the docs.
    #[test]
    fn the_api_key_path_is_offered_and_marked_unavailable() {
        let opts = model_options_for(&host(false, None, 16, 8), BOTH);
        let openai = opts
            .iter()
            .find(|o| o.target.model() == Some("openai:text-embedding-3-small"))
            .expect("the OpenAI option must be offered when remote-embed is compiled");
        assert!(
            openai.reason.contains("api_key"),
            "the option must name the field that holds the key, got: {}",
            openai.reason
        );
        assert!(
            !openai.available,
            "no key is probed for, so this can never be reported as ready to use"
        );
    }

    /// The public entry point reads this binary's own features. It cannot assert
    /// a specific model — that depends on the lane — but it can assert that it
    /// delegates rather than re-deriving a second answer.
    #[test]
    fn the_public_entry_point_agrees_with_the_compiled_backends() {
        let ctx = host(false, None, 16, 8);
        let opts = model_options_for_hardware(&ctx);
        let direct = model_options_for(&ctx, CompiledBackends::current());
        assert_eq!(opts.len(), direct.len());
        assert_eq!(opts[0].target, direct[0].target);
        assert!(!opts.is_empty());
    }

    #[test]
    fn ollama_tcp_addr_strips_http_prefix() {
        assert_eq!(ollama_tcp_addr("http://localhost:11434"), "localhost:11434");
        assert_eq!(ollama_tcp_addr("https://remote:11434"), "remote:11434");
        assert_eq!(ollama_tcp_addr("localhost:11434"), "localhost:11434");
        assert_eq!(ollama_tcp_addr("myhost"), "myhost:11434");
    }

    #[test]
    fn gpu_probe_skips_on_windows_without_optin() {
        use super::gpu_probe_enabled;
        // Windows + no opt-in → skip the subprocess probe (EDR/CreateProcessW hazard).
        assert!(!gpu_probe_enabled(true, false));
        // Windows + CODESCOUT_GPU_PROBE set → probe.
        assert!(gpu_probe_enabled(true, true));
        // Non-Windows → always probe, regardless of the opt-in flag.
        assert!(gpu_probe_enabled(false, false));
        assert!(gpu_probe_enabled(false, true));
    }
}
