//! Process-global registration of the statically-linked `sqlite-vec` (`vec0`)
//! extension as a SQLite auto-extension. Once registered, every rusqlite
//! `Connection` opened afterwards has `vec0` available.
//!
//! This lives outside `src/librarian/` (which is `cfg(feature = "librarian")`)
//! so the always-compiled retrieval code store can register it too. `vec0` is
//! **statically linked** into the binary — no runtime DLL, so nothing for an
//! EDR like CrowdStrike to quarantine (unlike the `onnxruntime.dll` of WIN-22).
//! That static-linking is what makes the daemon-free "lite" stack viable on a
//! locked-down VDI; see `docs/plans/2026-06-16-two-stack-retrieval-lite.md`.
//!
//! A single shared `Once` guarantees one registration regardless of which
//! subsystem (librarian catalog, retrieval code store, memory store) touches
//! sqlite-vec first — registering the same auto-extension twice would run the
//! `vec0` init on every connection twice.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Once;

// Compile-time pin on the upstream signature: if sqlite-vec ever changes the
// `sqlite3_vec_init` ABI, this fails to compile instead of mis-registering.
const _UPSTREAM_SQLITE_VEC_INIT_SIG: unsafe extern "C" fn() = sqlite_vec::sqlite3_vec_init;

static INIT: Once = Once::new();

/// Register `vec0` as a global SQLite auto-extension (idempotent, Once-guarded).
/// Call before opening any `Connection` that uses `vec0` virtual tables.
pub fn register() {
    INIT.call_once(|| {
        // SAFETY: sqlite3_vec_init is a valid SQLite extension entry point.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut i8,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> i32,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}
/// Map a project id to a filesystem-safe DB file stem (shared by the sqlite-vec
/// code + memory stores so a project always resolves to the same file).
pub fn sanitize_db_name(project_id: &str) -> String {
    let s: String = project_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "default".into()
    } else {
        s
    }
}

/// Open (once) and cache a project's sqlite-vec connection, creating its base
/// table from `ddl`. Shared by the code + memory sqlite-vec stores, which
/// differ only in db filename suffix and schema.
pub fn open_conn(
    dir: &Path,
    conns: &Mutex<HashMap<String, Arc<Mutex<Connection>>>>,
    project_id: &str,
    db_suffix: &str,
    ddl: &str,
) -> Result<Arc<Mutex<Connection>>> {
    let mut cache = conns.lock();
    if let Some(c) = cache.get(project_id) {
        return Ok(Arc::clone(c));
    }
    register();
    std::fs::create_dir_all(dir)
        .with_context(|| format!("create sqlite-vec dir {}", dir.display()))?;
    let path = dir.join(format!("{}{}", sanitize_db_name(project_id), db_suffix));
    let conn = Connection::open(&path)
        .with_context(|| format!("open sqlite-vec db {}", path.display()))?;
    conn.execute_batch(ddl).context("create sqlite-vec table")?;
    let arc = Arc::new(Mutex::new(conn));
    cache.insert(project_id.to_string(), Arc::clone(&arc));
    Ok(arc)
}

/// Little-endian f32 blob for a `vec0` embedding column / `vec_f32()` argument.
pub fn dense_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
