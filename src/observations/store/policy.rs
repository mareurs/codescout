//! A per-checkout, write-once opt-in latch outside the removable journal.
//! Linked worktrees use their own Git dir, never the common object directory.
use super::*;
const NAME: &str = "codescout-observation-policy-v1.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: u32,
    workspace: String,
    first_epoch: String,
}
pub(super) struct State {
    pub first_epoch: String,
    pub bytes_len: u64,
}
fn directory(root: &Path) -> Result<File, Diagnostic> {
    let repo = git2::Repository::discover(root).map_err(diag)?;
    let path = repo.path().canonicalize().map_err(diag)?;
    let dir = secure::root(&path)?;
    secure::supported(&dir)?;
    Ok(dir)
}
pub(super) fn load(root: &Path) -> Result<Option<State>, Diagnostic> {
    let dir = directory(root)?;
    if !secure::exists(&dir, NAME)? {
        return Ok(None);
    }
    let bytes = read_limited(&dir, NAME, 4096)?;
    let policy: Policy = serde_json::from_slice(&bytes).map_err(diag)?;
    if policy.schema_version != 1
        || Path::new(&policy.workspace) != root
        || uuid::Uuid::parse_str(&policy.first_epoch).is_err()
    {
        return Err("checkout-mismatch".into());
    }
    Ok(Some(State {
        first_epoch: policy.first_epoch,
        bytes_len: bytes.len() as u64,
    }))
}
pub(super) fn ensure(root: &Path, epoch: &str) -> Result<(), Diagnostic> {
    if load(root)?.is_some() {
        return Ok(());
    }
    let dir = directory(root)?;
    let policy = Policy {
        schema_version: 1,
        workspace: root.to_str().ok_or("unavailable")?.into(),
        first_epoch: epoch.into(),
    };
    let bytes = serde_json::to_vec(&policy).map_err(diag)?;
    if bytes.len() > 4096 {
        return Err("unavailable".into());
    }
    let file = secure::open(
        &dir,
        NAME,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        0o600,
        true,
        false,
    );
    let mut file = match file {
        Ok(file) => file,
        Err(e) => {
            // Another cooperating enable may have created the immutable latch.
            return if load(root)?.is_some() {
                Ok(())
            } else {
                Err(e)
            };
        }
    };
    file.write_all(&bytes).map_err(diag)?;
    file.sync_all().map_err(diag)?;
    dir.sync_all().map_err(diag)
}
