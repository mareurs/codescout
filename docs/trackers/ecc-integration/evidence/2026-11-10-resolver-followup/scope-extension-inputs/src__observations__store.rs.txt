//! Linux-only cooperative append store. Never interpreted as complete coverage.
use super::contract::{
    canonical_json, validate_observation, Caller, Diagnostic, Fingerprint, ObservationV1,
    TargetFact,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const HEALTH_BYTES: usize = 4096;
// cap-class: NOT_A_CAP — bounded private sequence registry; exhaustion reports quota rather than truncated evidence.
const MAX_CONTROL_BYTES: usize = 512 * 1024;
// cap-class: NOT_A_CAP — private journal decoding/append budget; exhaustion reports quota, never complete coverage.
const MAX_SHARDS: usize = 4096;
mod policy;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub record_bytes: usize,
    pub shard_bytes: usize,
    pub total_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            record_bytes: 64 * 1024,
            shard_bytes: 8 * 1024 * 1024,
            total_bytes: 64 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SanitizedEvent {
    pub schema_version: u32,
    pub event: String,
    pub producer_kind: String,
    pub workspace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_cwd: Option<String>,
    pub caller: Caller,
    pub tool: Option<String>,
    pub local_invocation: Option<String>,
    pub native_call_id: Option<String>,
    pub turn_id: Option<String>,
    pub fingerprints: Vec<Fingerprint>,
    pub targets: Vec<TargetFact>,
    pub outcome: String,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, Serialize)]
pub struct RecordReceipt {
    pub observation: ObservationV1,
}
pub enum StoreAvailability {
    Disabled,
    Enabled(Arc<ObservationStore>),
    Unavailable(Diagnostic),
}
#[derive(Debug, Default, Serialize)]
pub struct StoreRead {
    pub records: Vec<ObservationV1>,
    pub diagnostics: Vec<Diagnostic>,
    pub enabled: bool,
    pub store_epoch: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    schema_version: u32,
    workspace: String,
    epoch: String,
    enabled: bool,
    // Durable counts expose loss of an entire producer, not only holes in a surviving shard.
    producer_sequences: BTreeMap<String, u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    record: ObservationV1,
    checksum: String,
}
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Health {
    diagnostics: Vec<Diagnostic>,
}
struct Producer {
    id: String,
    sequence: u64,
    shard: u32,
    failed: bool,
}
pub struct ObservationStore {
    root: PathBuf,
    dir: File,
    epoch: String,
    limits: Limits,
    producer: Mutex<Producer>,
}

fn diag(_: impl std::fmt::Debug) -> Diagnostic {
    "unavailable".into()
}
pub fn checkout(path: &Path) -> Result<PathBuf, Diagnostic> {
    let repo = git2::Repository::discover(path).map_err(diag)?;
    let root = repo
        .workdir()
        .ok_or("unavailable")?
        .canonicalize()
        .map_err(diag)?;
    if root.to_str().is_none() {
        return Err("unavailable".into());
    }
    Ok(root)
}
#[cfg(target_os = "linux")]
mod secure {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::MetadataExt;
    pub fn root(path: &Path) -> Result<File, Diagnostic> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(diag)?;
        owned(&file, false, true)?;
        Ok(file)
    }
    fn owned(file: &File, private: bool, directory: bool) -> Result<(), Diagnostic> {
        let m = file.metadata().map_err(diag)?;
        // SAFETY: geteuid has no preconditions.
        if m.uid() != unsafe { libc::geteuid() }
            || m.file_type().is_dir() != directory
            || (!directory && !m.file_type().is_file())
            || (private && m.mode() & 0o077 != 0)
        {
            return Err("unavailable".into());
        }
        Ok(())
    }
    pub fn open(
        dir: &File,
        name: &str,
        flags: i32,
        mode: u32,
        private: bool,
        directory: bool,
    ) -> Result<File, Diagnostic> {
        let name = CString::new(name).map_err(diag)?;
        // SAFETY: name is NUL terminated, dir is live, flags describe the returned owned fd.
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                mode,
            )
        };
        if fd < 0 {
            return Err("unavailable".into());
        }
        // SAFETY: openat returned a new owned fd.
        let file = unsafe { File::from_raw_fd(fd) };
        owned(&file, private, directory)?;
        Ok(file)
    }
    pub fn child(dir: &File, name: &str, create: bool, private: bool) -> Result<File, Diagnostic> {
        if create {
            let name = CString::new(name).map_err(diag)?;
            // SAFETY: valid live dir fd and terminated relative name.
            let result = unsafe { libc::mkdirat(dir.as_raw_fd(), name.as_ptr(), 0o700) };
            if result < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
                return Err("unavailable".into());
            }
        }
        open(
            dir,
            name,
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
            private,
            true,
        )
    }
    pub fn exists(dir: &File, name: &str) -> Result<bool, Diagnostic> {
        let name = CString::new(name).map_err(diag)?;
        let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: s is writable and name is terminated; no symlink following.
        let rc = unsafe {
            libc::fstatat(
                dir.as_raw_fd(),
                name.as_ptr(),
                s.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if rc == 0 {
            return Ok(true);
        }
        if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
            Ok(false)
        } else {
            Err("unavailable".into())
        }
    }
    pub fn entries(dir: &File) -> Result<Vec<String>, Diagnostic> {
        // SAFETY: duplication supplies fdopendir an independently owned descriptor.
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                c".".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err("unavailable".into());
        }
        // SAFETY: fd is a newly owned directory fd; closed by closedir below.
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            // SAFETY: ownership was not transferred when fdopendir failed.
            unsafe { libc::close(fd) };
            return Err("unavailable".into());
        }
        let mut result = Vec::new();
        loop {
            // SAFETY: stream remains live until closedir, names are copied before the next call.
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                break;
            }
            // SAFETY: readdir returns a terminated d_name while stream is live.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            if name != "." && name != ".." {
                result.push(name);
            }
            if result.len() > MAX_SHARDS + 3 {
                break;
            }
        }
        // SAFETY: stream was successfully created and is closed once.
        unsafe { libc::closedir(stream) };
        if result.len() > MAX_SHARDS + 3 {
            return Err("quota".into());
        }
        result.sort();
        Ok(result)
    }
    pub fn rename(dir: &File, from: &str, to: &str) -> Result<(), Diagnostic> {
        let from = CString::new(from).map_err(diag)?;
        let to = CString::new(to).map_err(diag)?;
        // SAFETY: both relative names and directory fd remain valid for the call.
        if unsafe { libc::renameat(dir.as_raw_fd(), from.as_ptr(), dir.as_raw_fd(), to.as_ptr()) }
            < 0
        {
            return Err("unavailable".into());
        }
        dir.sync_all().map_err(diag)
    }
    pub fn supported(dir: &File) -> Result<(), Diagnostic> {
        let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
        // SAFETY: stat points to writable storage, dir is live.
        if unsafe { libc::fstatfs(dir.as_raw_fd(), stat.as_mut_ptr()) } < 0 {
            return Err("unavailable".into());
        }
        // SAFETY: fstatfs initialized stat on success.
        let kind = unsafe { stat.assume_init() }.f_type as u64;
        if ![0xef53, 0x01021994, 0x9123683e, 0x58465342, 0x794c7630].contains(&kind) {
            return Err("unavailable".into());
        }
        Ok(())
    }
    pub fn same_dir(a: &File, b: &File) -> Result<bool, Diagnostic> {
        let a = a.metadata().map_err(diag)?;
        let b = b.metadata().map_err(diag)?;
        Ok(a.dev() == b.dev() && a.ino() == b.ino())
    }

    pub struct Lock {
        _file: File,
    }
    pub fn lock(dir: &File, create: bool, exclusive: bool) -> Result<Lock, Diagnostic> {
        let file = open(
            dir,
            "quota.lock",
            if create {
                libc::O_RDWR | libc::O_CREAT
            } else {
                libc::O_RDONLY
            },
            0o600,
            true,
            false,
        )?;
        let started = std::time::Instant::now();
        loop {
            // SAFETY: flock operates on an owned live fd, unlocked when it closes.
            if unsafe {
                libc::flock(
                    file.as_raw_fd(),
                    (if exclusive {
                        libc::LOCK_EX
                    } else {
                        libc::LOCK_SH
                    }) | libc::LOCK_NB,
                )
            } == 0
            {
                return Ok(Lock { _file: file });
            }
            if started.elapsed() >= std::time::Duration::from_millis(100) {
                return Err("unavailable".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}
#[cfg(not(target_os = "linux"))]
compile_error!("observation store requires a platform implementation; gate its module on Linux");

fn directory(root: &Path, create: bool) -> Result<Option<File>, Diagnostic> {
    let r = secure::root(root)?;
    if !create && !secure::exists(&r, ".codescout")? {
        return Ok(None);
    }
    let cs = secure::child(&r, ".codescout", create, false)?;
    if !create && !secure::exists(&cs, "observations")? {
        return Ok(None);
    }
    let observations = secure::child(&cs, "observations", create, true)?;
    // A self-ignored rule keeps capture private in arbitrary checkouts without
    // editing the user's tracked ignore file or shared Git configuration.
    if create && !secure::exists(&observations, ".gitignore")? {
        let mut file = secure::open(
            &observations,
            ".gitignore",
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
            true,
            false,
        )?;
        file.write_all(b"*\n").map_err(diag)?;
        file.sync_all().map_err(diag)?;
        observations.sync_all().map_err(diag)?;
    }
    if read_limited(&observations, ".gitignore", 2)? != b"*\n" {
        return Err("unavailable".into());
    }
    if !create && !secure::exists(&observations, "v1")? {
        return Ok(None);
    }
    let dir = secure::child(&observations, "v1", create, true)?;
    secure::supported(&dir)?;
    Ok(Some(dir))
}
fn read_limited(dir: &File, name: &str, limit: usize) -> Result<Vec<u8>, Diagnostic> {
    let f = secure::open(dir, name, libc::O_RDONLY, 0, true, false)?;
    if f.metadata().map_err(diag)?.len() > limit as u64 {
        return Err("unavailable".into());
    }
    let mut data = Vec::new();
    f.take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(diag)?;
    if data.len() > limit {
        return Err("unavailable".into());
    }
    Ok(data)
}
fn control(dir: &File, root: &Path) -> Result<Control, Diagnostic> {
    let c: Control = serde_json::from_slice(&read_limited(dir, "control.json", MAX_CONTROL_BYTES)?)
        .map_err(diag)?;
    if c.schema_version != 1
        || Path::new(&c.workspace) != root
        || uuid::Uuid::parse_str(&c.epoch).is_err()
    {
        return Err("checkout-mismatch".into());
    }
    if c.producer_sequences.len() > MAX_SHARDS
        || c.producer_sequences.iter().any(|(id, count)| {
            uuid::Uuid::parse_str(id)
                .ok()
                .is_none_or(|u| u.to_string() != *id)
                || !(1..=9007199254740991).contains(count)
        })
    {
        return Err("incompatible-schema".into());
    }
    Ok(c)
}
fn write_control(dir: &File, c: &Control) -> Result<(), Diagnostic> {
    let bytes = serde_json::to_vec(c).map_err(diag)?;
    if bytes.len() > MAX_CONTROL_BYTES {
        return Err("quota".into());
    }
    let name = format!(".control-{}.tmp", uuid::Uuid::new_v4());
    let mut f = secure::open(
        dir,
        &name,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        0o600,
        true,
        false,
    )?;
    f.write_all(&bytes).map_err(diag)?;
    f.sync_all().map_err(diag)?;
    secure::rename(dir, &name, "control.json")
}
fn health(dir: &File, code: &str) -> Result<(), Diagnostic> {
    let bytes = read_limited(dir, "health.json", HEALTH_BYTES)?;
    let mut h: Health = serde_json::from_slice(&bytes).map_err(diag)?;
    if !h.diagnostics.iter().any(|d| d == code) {
        h.diagnostics.push(code.into());
    }
    let mut bytes = serde_json::to_vec(&h).map_err(diag)?;
    if bytes.len() > HEALTH_BYTES {
        return Err("quota".into());
    }
    bytes.resize(HEALTH_BYTES, b' ');
    let mut f = secure::open(dir, "health.json", libc::O_WRONLY, 0, true, false)?;
    f.seek(SeekFrom::Start(0)).map_err(diag)?;
    f.write_all(&bytes).map_err(diag)?;
    f.sync_all().map_err(diag)
}
pub fn enable(root: &Path) -> Result<String, Diagnostic> {
    let root = checkout(root)?;
    let dir = directory(&root, true)?.ok_or("unavailable")?;
    let _lock = secure::lock(&dir, true, true)?;
    let mut c = if secure::exists(&dir, "control.json")? {
        control(&dir, &root)?
    } else {
        if secure::entries(&dir)?.iter().any(|n| n != "quota.lock") {
            return Err("unavailable".into());
        }
        let mut h = serde_json::to_vec(&Health::default()).map_err(diag)?;
        h.resize(HEALTH_BYTES, b' ');
        let mut file = secure::open(
            &dir,
            "health.json",
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
            true,
            false,
        )?;
        file.write_all(&h).map_err(diag)?;
        file.sync_all().map_err(diag)?;
        Control {
            schema_version: 1,
            workspace: root.to_string_lossy().into_owned(),
            epoch: uuid::Uuid::new_v4().to_string(),
            enabled: true,
            producer_sequences: BTreeMap::new(),
        }
    };
    policy::ensure(&root, &c.epoch)?;
    c.enabled = true;
    write_control(&dir, &c)?;
    Ok(c.epoch)
}
pub fn disable(root: &Path) -> Result<(), Diagnostic> {
    let root = checkout(root)?;
    let Some(dir) = directory(&root, false)? else {
        return Ok(());
    };
    if !secure::exists(&dir, "control.json")? {
        return Ok(());
    }
    let _lock = secure::lock(&dir, false, true)?;
    let mut c = control(&dir, &root)?;
    c.enabled = false;
    write_control(&dir, &c)
}
impl ObservationStore {
    pub fn open_if_enabled(root: &Path, limits: Limits) -> StoreAvailability {
        let open = || -> Result<Option<Self>, Diagnostic> {
            let root = checkout(root)?;
            let policy = policy::load(&root)?;
            let Some(dir) = directory(&root, false)? else {
                return if policy.is_some() {
                    Err("reset".into())
                } else {
                    Ok(None)
                };
            };
            if !secure::exists(&dir, "control.json")? {
                return if policy.is_some() {
                    Err("reset".into())
                } else {
                    Ok(None)
                };
            }
            if policy.is_none() {
                return Err("reset".into());
            }
            let c = control(&dir, &root)?;
            if !c.enabled {
                return Ok(None);
            }
            Ok(Some(Self {
                root,
                dir,
                epoch: c.epoch,
                limits,
                producer: Mutex::new(Producer {
                    id: uuid::Uuid::new_v4().to_string(),
                    sequence: 0,
                    shard: 0,
                    failed: false,
                }),
            }))
        };
        match open() {
            Ok(Some(s)) => StoreAvailability::Enabled(Arc::new(s)),
            Ok(None) => StoreAvailability::Disabled,
            Err(d) => StoreAvailability::Unavailable(d),
        }
    }
    pub fn append(&self, event: SanitizedEvent) -> Result<RecordReceipt, Diagnostic> {
        let mut producer = self.producer.lock();
        if producer.failed {
            return Err("unavailable".into());
        }
        let _lock = secure::lock(&self.dir, false, true)?;
        let current = directory(&self.root, false)?.ok_or("unavailable")?;
        if !secure::same_dir(&self.dir, &current)? {
            return Err("checkout-mismatch".into());
        }
        let mut c = control(&self.dir, &self.root)?;
        if !c.enabled {
            return Err("disabled".into());
        }
        if c.epoch != self.epoch {
            return Err("reset".into());
        }
        if event.workspace != self.root.to_string_lossy() {
            return Err("checkout-mismatch".into());
        }
        let mut v = serde_json::to_value(&event).map_err(diag)?;
        v["observation_id"] = Value::String(format!("{}:{}", producer.id, producer.sequence));
        v["producer_instance"] = Value::String(producer.id.clone());
        v["sequence"] = producer.sequence.into();
        v["store_epoch"] = self.epoch.clone().into();
        let observed_at: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(diag)?
            .as_millis()
            .try_into()
            .map_err(diag)?;
        v["observed_at_unix_ms"] = observed_at.into();
        let record = validate_observation(v).map_err(str::to_owned)?;
        let checksum = hex::encode(Sha256::digest(
            canonical_json(&serde_json::to_value(&record).map_err(diag)?)
                .map_err(str::to_owned)?
                .as_bytes(),
        ));
        let mut bytes = serde_json::to_vec(&Line {
            record: record.clone(),
            checksum,
        })
        .map_err(diag)?;
        bytes.push(b'\n');
        if bytes.len() > self.limits.record_bytes || bytes.len() > self.limits.shard_bytes {
            return Err("incomplete-input".into());
        }
        if c.producer_sequences.get(&producer.id).copied().unwrap_or(0) != producer.sequence {
            return Err("sequence-gap".into());
        }
        let next_sequence = producer.sequence.checked_add(1).ok_or("quota")?;
        if next_sequence > 9007199254740991 {
            return Err("quota".into());
        }
        c.producer_sequences
            .insert(producer.id.clone(), next_sequence);
        let control_bytes = serde_json::to_vec(&c).map_err(diag)?.len();
        if control_bytes > MAX_CONTROL_BYTES || c.producer_sequences.len() > MAX_SHARDS {
            let _ = health(&self.dir, "quota");
            return Err("quota".into());
        }
        let entries = secure::entries(&self.dir)?;
        let policy = policy::load(&self.root)?.ok_or("reset")?;
        // Include the immutable latch and the two-byte self-ignore rule in the checkout cap.
        let mut total = policy.bytes_len.saturating_add(2);
        for entry in &entries {
            let f = secure::open(&self.dir, entry, libc::O_RDONLY, 0, true, false)?;
            total = total.saturating_add(f.metadata().map_err(diag)?.len());
        }
        if entries.len() >= MAX_SHARDS + 3
            || total
                .saturating_add(bytes.len() as u64)
                .saturating_add(control_bytes as u64)
                > self.limits.total_bytes as u64
        {
            let _ = health(&self.dir, "quota");
            return Err("quota".into());
        }
        let mut name = format!("{}-{:06}.jsonl", producer.id, producer.shard);
        if secure::exists(&self.dir, &name)? {
            let f = secure::open(&self.dir, &name, libc::O_RDONLY, 0, true, false)?;
            if f.metadata().map_err(diag)?.len() + bytes.len() as u64
                > self.limits.shard_bytes as u64
            {
                producer.shard += 1;
                name = format!("{}-{:06}.jsonl", producer.id, producer.shard);
            }
        }
        let mut file = secure::open(
            &self.dir,
            &name,
            libc::O_WRONLY | libc::O_APPEND | libc::O_CREAT,
            0o600,
            true,
            false,
        )?;
        producer.sequence += 1;
        if file
            .write_all(&bytes)
            .and_then(|_| file.sync_all())
            .and_then(|_| self.dir.sync_all())
            .map_err(diag)
            .and_then(|_| write_control(&self.dir, &c))
            .is_err()
        {
            producer.failed = true;
            let _ = health(&self.dir, "unavailable");
            return Err("unavailable".into());
        }
        Ok(RecordReceipt {
            observation: record,
        })
    }
}
pub fn read_store(root: &Path, limits: Limits) -> StoreRead {
    let mut result = StoreRead::default();
    let read = |result: &mut StoreRead| -> Result<(), Diagnostic> {
        let root = checkout(root)?;
        let policy = policy::load(&root)?;
        let Some(dir) = directory(&root, false)? else {
            if policy.is_some() {
                return Err("reset".into());
            }
            result.diagnostics.push("disabled".into());
            return Ok(());
        };
        if !secure::exists(&dir, "control.json")? {
            if policy.is_some() {
                return Err("reset".into());
            }
            result.diagnostics.push("disabled".into());
            return Ok(());
        }
        let _lock = secure::lock(&dir, false, false)?;
        let c = control(&dir, &root)?;
        if policy.as_ref().is_none_or(|p| p.first_epoch != c.epoch) {
            result.diagnostics.push("reset".into());
        }
        result.enabled = c.enabled;
        result.store_epoch = Some(c.epoch.clone());
        let h: Health = serde_json::from_slice(&read_limited(&dir, "health.json", HEALTH_BYTES)?)
            .map_err(diag)?;
        if h.diagnostics.len() > 32 {
            return Err("unavailable".into());
        }
        result.diagnostics.extend(h.diagnostics);
        let mut seen: HashMap<String, Vec<u8>> = HashMap::new();
        let mut sequences: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        let mut total = policy.as_ref().map_or(0, |p| p.bytes_len) as usize + 2;
        for name in ["control.json", "health.json", "quota.lock"] {
            total = total.saturating_add(
                secure::open(&dir, name, libc::O_RDONLY, 0, true, false)?
                    .metadata()
                    .map_err(diag)?
                    .len() as usize,
            );
        }
        if total > limits.total_bytes {
            return Err("quota".into());
        }
        for name in secure::entries(&dir)? {
            if ["control.json", "quota.lock", "health.json"].contains(&name.as_str()) {
                continue;
            }
            if !name.ends_with(".jsonl") {
                result.diagnostics.push("unavailable".into());
                continue;
            }
            let bytes = read_limited(&dir, &name, limits.shard_bytes)?;
            total = total.saturating_add(bytes.len());
            if total > limits.total_bytes {
                return Err("quota".into());
            }
            if bytes.is_empty() || !bytes.ends_with(b"\n") {
                result.diagnostics.push("torn-record".into());
            }
            for line in bytes.split_inclusive(|b| *b == b'\n') {
                if line.len() > limits.record_bytes || !line.ends_with(b"\n") {
                    result.diagnostics.push("torn-record".into());
                    continue;
                }
                let parsed = || -> Result<ObservationV1, Diagnostic> {
                    let raw: Value = serde_json::from_slice(line).map_err(diag)?;
                    let wrapper: Line = serde_json::from_value(raw.clone()).map_err(diag)?;
                    let record =
                        validate_observation(raw["record"].clone()).map_err(str::to_owned)?;
                    let canonical = canonical_json(&raw["record"]).map_err(str::to_owned)?;
                    if wrapper.checksum != hex::encode(Sha256::digest(canonical.as_bytes())) {
                        return Err("checksum".into());
                    }
                    if record.workspace != root.to_string_lossy() || record.store_epoch != c.epoch {
                        return Err("checkout-mismatch".into());
                    }
                    Ok(record)
                };
                match parsed() {
                    Err(d) => result.diagnostics.push(d),
                    Ok(record) => {
                        if let Some(old) = seen.get(&record.observation_id) {
                            if old != line {
                                result.diagnostics.push("conflicting-duplicate".into());
                            }
                            continue;
                        }
                        seen.insert(record.observation_id.clone(), line.to_vec());
                        sequences
                            .entry(record.producer_instance.clone())
                            .or_default()
                            .push(record.sequence);
                        result.records.push(record);
                    }
                }
            }
        }
        if sequences.keys().ne(c.producer_sequences.keys()) {
            result.diagnostics.push("sequence-gap".into());
        }
        for (producer, values) in &mut sequences {
            values.sort_unstable();
            if c.producer_sequences.get(producer).copied() != Some(values.len() as u64)
                || values.iter().copied().ne(0..values.len() as u64)
            {
                result.diagnostics.push("sequence-gap".into());
            }
        }
        result.records.sort_by(|a, b| {
            (&a.producer_instance, a.sequence).cmp(&(&b.producer_instance, b.sequence))
        });
        Ok(())
    };
    if let Err(d) = read(&mut result) {
        result.diagnostics.push(d);
    }
    result.diagnostics.sort();
    result.diagnostics.dedup();
    result
}
