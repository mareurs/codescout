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
