use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// How one passwd lookup ended. `Failed` is NOT cached (a transient error must not switch the
/// account guard off for the life of the process); `Found` and `NoEntry` are definitive.
#[derive(Debug, PartialEq, Eq)]
enum AccountLookup {
    Found(PathBuf),
    /// The database has no entry for this user, or the entry has no absolute `pw_dir`.
    NoEntry,
    /// The call failed for a reason that says nothing about the entry (out of memory, I/O
    /// error, signals never stopped interrupting it, a buffer that would not stop growing).
    Failed,
}

/// The effective user's home directory from the account database (`getpwuid_r`), independent of
/// `$HOME`. `None` when the account has no entry or no usable (absolute) `pw_dir`.
///
/// Security guards use this beside `$HOME`: `$HOME` is an environment variable, and an unset,
/// empty or wrong one must not make a guard that is "about the home directory" silently stop
/// guarding the real one.
///
/// Cached, because every write classification and read validation asks for it and one lookup can
/// mean a blocking NSS round trip (LDAP, sssd) on whatever thread asked. A definitive answer
/// (found, or no entry) is kept for the life of the process. A FAILED lookup (EAGAIN, EIO: an NSS
/// outage) is remembered for `PASSWD_FAILURE_TTL` and not repeated before then, so an outage costs
/// one blocking call per window, not one per tool call; after the window it is tried again, so a
/// transient failure cannot switch the account guard off for good. The cache holds the ACCOUNT's
/// home only; `$HOME` is read fresh on every call by the caller (`platform::resolved_home_dir`).
/// An account's home directory changing under a running server is not handled: restart it.
///
/// Where it runs: `protected_homes` calls this from synchronous code reached from async tool
/// handlers, so the FIRST lookup (and a retry after a failure window) runs on a tokio worker thread
/// and can block it for the length of one NSS call. It is not moved to `spawn_blocking`: the call
/// sites are plain functions with no async context to await in, and the cache bounds the cost to one
/// call per process (or per failure window).
pub fn account_home_dir() -> Option<PathBuf> {
    static ACCOUNT_HOME: AccountCache = AccountCache::new();
    cached_account_home(&ACCOUNT_HOME, std::time::Instant::now, lookup_account_home)
}

/// How long a failed account lookup is not repeated.
// cap-class: NOT_A_CAP — a retry back-off for one account lookup; it bounds how often the lookup is tried, and no result is truncated or presented as complete
const PASSWD_FAILURE_TTL: std::time::Duration = std::time::Duration::from_secs(5);

/// The account lookup's memory: a definitive answer kept for the life of the process, and when the
/// last attempt FAILED. The mutex is held across the lookup on purpose, so concurrent first callers
/// make one NSS call between them rather than one each.
struct AccountCache {
    known: std::sync::OnceLock<Option<PathBuf>>,
    failed_at: std::sync::Mutex<Option<std::time::Instant>>,
}

impl AccountCache {
    const fn new() -> Self {
        Self {
            known: std::sync::OnceLock::new(),
            failed_at: std::sync::Mutex::new(None),
        }
    }
}

/// The cache policy of `account_home_dir` over a caller-supplied cache, clock and lookup: a
/// definitive answer (`Found`, `NoEntry`) is asked once and kept; a `Failed` one is remembered for
/// `PASSWD_FAILURE_TTL` and the lookup is not run again before it has passed. `now` is injectable
/// so a test can move time without sleeping.
fn cached_account_home(
    cache: &AccountCache,
    now: impl Fn() -> std::time::Instant,
    lookup: impl FnOnce() -> AccountLookup,
) -> Option<PathBuf> {
    if let Some(known) = cache.known.get() {
        return known.clone();
    }
    // Held across the lookup: concurrent callers wait for this one instead of each asking NSS.
    let mut failed_at = cache.failed_at.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(known) = cache.known.get() {
        return known.clone(); // another caller finished while this one waited
    }
    if let Some(at) = *failed_at {
        if now().saturating_duration_since(at) < PASSWD_FAILURE_TTL {
            return None;
        }
    }
    match lookup() {
        AccountLookup::Found(home) => cache.known.get_or_init(|| Some(home)).clone(),
        AccountLookup::NoEntry => cache.known.get_or_init(|| None).clone(),
        AccountLookup::Failed => {
            *failed_at = Some(now());
            None
        }
    }
}

/// Upper bound on the `getpwuid_r` scratch buffer: a 1 MiB entry is not a passwd entry.
// cap-class: NOT_A_CAP — scratch-buffer ceiling for one account lookup; a breach ends the lookup as AccountLookup::Failed (the home guard then relies on $HOME), no result is truncated or presented as complete
const PASSWD_BUF_MAX: usize = 1 << 20;
/// Upper bound on EINTR retries of one lookup.
const PASSWD_EINTR_RETRIES: usize = 8;

/// `getpwuid_r` for the effective uid, over the real libc call.
///
/// `geteuid`, not `getuid`: file access is authorised by the EFFECTIVE uid, so the home directory
/// worth guarding is that user's. `std::env::home_dir` and `dirs-sys` use `getuid`; the two are
/// equal for a process that is not setuid, which a stdio MCP server is not, so this is a choice
/// of which to follow in a case that does not arise, not a behavioural difference.
fn lookup_account_home() -> AccountLookup {
    // SAFETY: `geteuid` has no preconditions and cannot fail.
    let uid = unsafe { libc::geteuid() };
    let mut real_getpwuid_r =
        |pwd: &mut libc::passwd, buf: &mut [u8], result: &mut *mut libc::passwd| {
            // SAFETY: `pwd`, `buf` and `result` are exclusive references that outlive the call;
            // `buf.len()` is the real length of the buffer the pointer addresses; `getpwuid_r` is the
            // reentrant variant and stores its string fields inside `buf`.
            unsafe {
                libc::getpwuid_r(
                    uid,
                    pwd,
                    buf.as_mut_ptr().cast::<libc::c_char>(),
                    buf.len(),
                    result,
                )
            }
        };
    // SAFETY: the closure forwards its own `pwd`, `buf` and `result` to `getpwuid_r`, which honours
    // the contract `lookup_account_home_with` requires: on success it stores the `pw_dir` string
    // inside the `buf` it was given, NUL-terminated.
    unsafe { lookup_account_home_with(&mut real_getpwuid_r) }
}

/// The retry and interpretation logic of `lookup_account_home`, over a `getpwuid_r`-shaped
/// call so a test can drive EINTR, ERANGE and the no-entry shapes without a passwd database.
///
/// # Safety
///
/// `call` must honour the `getpwuid_r(3)` contract for the `pwd`, `buf` and `result` it is handed:
/// when it returns `0` having stored a non-null pointer in `*result`, `pwd.pw_dir` must be null or
/// point at a NUL-terminated string that lies inside the `buf` passed in the SAME call. This
/// function reads that string with `CStr::from_ptr` and nothing else checks it; a closure that
/// stores a dangling, unterminated or foreign pointer is undefined behaviour here.
unsafe fn lookup_account_home_with(
    call: &mut dyn FnMut(&mut libc::passwd, &mut [u8], &mut *mut libc::passwd) -> libc::c_int,
) -> AccountLookup {
    use std::os::unix::ffi::OsStringExt;

    let mut buf = vec![0u8; 1024];
    let mut interrupted = 0;
    loop {
        // SAFETY: `libc::passwd` is a plain C struct of integers and raw pointers; the all-zero
        // pattern (null pointers, zero ids) is a valid value of it.
        let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        match call(&mut pwd, &mut buf, &mut result) {
            0 => {
                if result.is_null() || pwd.pw_dir.is_null() {
                    return AccountLookup::NoEntry;
                }
                // SAFETY: `pw_dir` is non-null and, because the call returned 0 with a non-null
                // `result`, the caller's contract (this function's `# Safety`) makes it a
                // NUL-terminated string inside `buf`, which is alive and not touched again until
                // these bytes are copied out here.
                let bytes = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) }
                    .to_bytes()
                    .to_vec();
                let dir = PathBuf::from(std::ffi::OsString::from_vec(bytes));
                return if dir.is_absolute() {
                    AccountLookup::Found(dir)
                } else {
                    AccountLookup::NoEntry
                };
            }
            libc::EINTR if interrupted < PASSWD_EINTR_RETRIES => interrupted += 1,
            libc::ERANGE if buf.len() < PASSWD_BUF_MAX => {
                let doubled = buf.len() * 2;
                buf.resize(doubled, 0);
            }
            // POSIX lets an implementation report "no such user" as an error code instead of
            // `0` with a null `result`.
            libc::ENOENT | libc::ESRCH => return AccountLookup::NoEntry,
            _ => return AccountLookup::Failed,
        }
    }
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn denied_read_prefixes() -> &'static [&'static str] {
    &[
        // Cloud / provider credentials
        "~/.ssh",
        "~/.aws",
        "~/.gnupg",
        "~/.config/gcloud",
        "~/.config/gh",
        "~/.docker/config.json",
        "~/.netrc",
        "~/.npmrc",
        "~/.kube/config",
        // Git credential stores (both legacy and XDG locations)
        "~/.git-credentials",
        "~/.config/git/credentials",
        // Package-registry credentials
        "~/.pypirc",
        "~/.cargo/credentials.toml",
        "~/.cargo/credentials",
        // DB + SQL client credentials
        "~/.pgpass",
        "~/.my.cnf",
        // Password managers / keyrings
        "~/.password-store",
        "~/.config/op",
        "~/.config/Bitwarden",
        "~/.local/share/keyrings",
        // Shell/tool history — often captures secret argv
        "~/.bash_history",
        "~/.zsh_history",
        "~/.psql_history",
        "~/.python_history",
        "~/.config/atuin",
        // macOS: Keychain stores
        "~/Library/Keychains",
        // System secrets (Linux)
        "/etc/shadow",
        "/etc/gshadow",
        "/etc/sudoers",
        "/etc/sudoers.d",
        // macOS system secrets
        "/etc/master.passwd",
        "/private/etc/sudoers",
        "/private/etc/sudoers.d",
        "/private/etc/master.passwd",
        // Linux: /proc/self/environ and /proc/self/mem leak the current
        // process's env and memory. Only self is predictable without a pid
        // glob; deny-list format does not support glob so we block the most
        // directly reachable patterns.
        "/proc/self/environ",
        "/proc/self/mem",
    ]
}

/// Always `None` — POSIX guarantees `/bin/sh`. The Windows twin can answer
/// `Some(hint)` when no Git Bash is installed.
pub fn shell_unavailable_hint() -> Option<String> {
    None
}

pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    let mut c = tokio::process::Command::new("sh");
    c.arg("-c")
        .arg(cmd)
        .env("GIT_PAGER", "cat")
        .stdin(std::process::Stdio::null())
        .process_group(0);
    // SAFETY: pre_exec runs post-fork, pre-exec; signal() is async-signal-safe.
    unsafe {
        c.pre_exec(|| {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
            Ok(())
        });
    }
    c
}

/// POSIX tokenization, shared with the Windows implementation now that both
/// platforms execute through a POSIX shell.
pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    super::posix_tokenize(cmd)
}

/// Narrow a `u32` to a pid that addresses **one process**, or `None`.
///
/// `libc::kill` takes a signed `pid_t`, and the sign is not a formality — it
/// selects a different addressing mode entirely:
///
/// | value | what `kill` addresses |
/// |---|---|
/// | `> 0` | that one process |
/// | `0` | every process in the **caller's own process group** |
/// | `-1` | every process the caller has permission to signal |
/// | `< -1` | every process in process group `-pid` |
///
/// So `pid as i32` is not a widening formality. Any `u32` at or above `2^31`
/// wraps to a negative value and silently changes what the call means:
/// [`process_alive`] returned **`true` for a process that does not exist**
/// (`kill(-1, 0)` succeeds), and [`terminate_process`] would have sent SIGTERM
/// to every process the user owns.
///
/// Kernel-assigned pids fit in `i32` — Linux caps `pid_max` at 4,194,304 — so a
/// pid taken straight from `Child::id()` can never reach these cases, which is
/// what the safety comment at `lsp/client.rs` correctly claims of *its* caller.
/// A pid **parsed** from a file, a JSON payload or a catalog row can, and this
/// crate now has both kinds: `librarian::reindex_progress` reads pids back out
/// of `catalog_meta`. Validating here rather than at each call site is the
/// difference between one check and N.
///
/// Measured 2026-09-05: `process_alive(u32::MAX)` returned `true` on Linux, and
/// `process_alive(0)` returns `true` for any caller in a live process group —
/// the latter known since 2026-08-18 and worked around at one call site
/// (`tools/rendezvous.rs`) rather than fixed here.
fn addressable_pid(pid: u32) -> Option<libc::pid_t> {
    match libc::pid_t::try_from(pid) {
        Ok(p) if p > 0 => Some(p),
        _ => None,
    }
}

pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    // Refused rather than cast: see `addressable_pid`. The cast would turn a
    // single-process SIGTERM into a group- or session-wide one.
    let Some(target) = addressable_pid(pid) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{pid} does not address a single process"),
        ));
    };
    let ret = unsafe { libc::kill(target, libc::SIGTERM) };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub fn process_alive(pid: u32) -> bool {
    // `false` rather than a cast: a pid that cannot address one process is not
    // a live process, and answering `true` — which `kill(-1, 0)` and
    // `kill(0, 0)` both do — makes a dead holder look alive to every liveness
    // check in the tree. See `addressable_pid`.
    let Some(target) = addressable_pid(pid) else {
        return false;
    };
    unsafe { libc::kill(target, 0) == 0 }
}

pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

pub fn lsp_binary_name(base: &str) -> String {
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_some() {
        assert!(home_dir().is_some());
    }

    /// `account_home_dir` against the passwd file, where the file names this process's user. When
    /// the file does not (LDAP, a container uid with no entry) there is nothing to compare: the test
    /// SAYS it was skipped (stderr) rather than passing silently, and the lookup is then unproven on
    /// that host. The logic around the lookup (retry, ERANGE, cache, TTL) is tested without a passwd
    /// entry by the injected-closure tests, and the guards that USE the account home by the
    /// injected-account tests in `util::path_security`.
    #[test]
    fn account_home_dir_matches_the_passwd_entry_for_this_user() {
        // SAFETY: `geteuid` has no preconditions and cannot fail.
        let uid = unsafe { libc::geteuid() }.to_string();
        let Ok(passwd) = std::fs::read_to_string("/etc/passwd") else {
            eprintln!("SKIPPED: /etc/passwd is unreadable; the real lookup is unproven here");
            return;
        };
        let Some(expected) = passwd.lines().find_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            (f.len() >= 6 && f[2] == uid).then(|| PathBuf::from(f[5]))
        }) else {
            eprintln!(
                "SKIPPED: uid {uid} has no /etc/passwd entry; the real lookup is unproven here"
            );
            return;
        };
        // A non-absolute `pw_dir` is not a home directory: the lookup reports none.
        let expected = expected.is_absolute().then_some(expected);
        assert_eq!(account_home_dir(), expected);
    }

    /// A fake `getpwuid_r` that reports `dir` as the home directory: the string goes into the
    /// scratch buffer, as the real call does, and `pw_dir` points at it.
    fn fill_entry(
        dir: &str,
        pwd: &mut libc::passwd,
        buf: &mut [u8],
        result: &mut *mut libc::passwd,
    ) -> libc::c_int {
        let bytes = dir.as_bytes();
        if buf.len() < bytes.len() + 1 {
            return libc::ERANGE;
        }
        buf[..bytes.len()].copy_from_slice(bytes);
        buf[bytes.len()] = 0;
        pwd.pw_dir = buf.as_mut_ptr().cast::<libc::c_char>();
        *result = pwd;
        0
    }

    /// `lookup_account_home_with` is `unsafe`: its closure must honour the `getpwuid_r` contract.
    /// Every closure in this module does: it reports no entry and leaves `pw_dir` null, or it is
    /// `fill_entry`, which NUL-terminates the string inside the buffer it was handed and points
    /// `pw_dir` at it (the "scribbled" case leaves `result` null, so the pointer is never read).
    fn lookup(
        call: &mut dyn FnMut(&mut libc::passwd, &mut [u8], &mut *mut libc::passwd) -> libc::c_int,
    ) -> AccountLookup {
        // SAFETY: see the doc comment of this helper; no closure passed to it stores a pointer
        // that is not inside the buffer of the same call.
        unsafe { lookup_account_home_with(call) }
    }

    #[test]
    fn a_lookup_interrupted_by_a_signal_is_retried() {
        let mut calls = 0;
        let found = lookup(&mut |pwd, buf, result| {
            calls += 1;
            if calls <= 2 {
                return libc::EINTR;
            }
            fill_entry("/home/fake", pwd, buf, result)
        });
        assert_eq!(found, AccountLookup::Found(PathBuf::from("/home/fake")));
        assert_eq!(calls, 3);
    }

    #[test]
    fn a_lookup_that_is_always_interrupted_gives_up_after_the_retry_bound() {
        let mut calls = 0;
        let found = lookup(&mut |_, _, _| {
            calls += 1;
            libc::EINTR
        });
        assert_eq!(found, AccountLookup::Failed);
        assert_eq!(
            calls,
            1 + PASSWD_EINTR_RETRIES,
            "one attempt plus the retries"
        );
    }

    #[test]
    fn a_too_small_buffer_is_grown_and_the_lookup_retried() {
        let mut sizes = Vec::new();
        let found = lookup(&mut |pwd, buf, result| {
            sizes.push(buf.len());
            if buf.len() < 4096 {
                return libc::ERANGE;
            }
            fill_entry("/home/fake", pwd, buf, result)
        });
        assert_eq!(found, AccountLookup::Found(PathBuf::from("/home/fake")));
        assert_eq!(sizes, vec![1024, 2048, 4096]);
    }

    #[test]
    fn a_buffer_that_never_suffices_stops_growing_at_the_bound() {
        let mut last = 0;
        let found = lookup(&mut |_, buf, _| {
            last = buf.len();
            libc::ERANGE
        });
        assert_eq!(found, AccountLookup::Failed);
        assert_eq!(last, PASSWD_BUF_MAX);
    }

    #[test]
    fn every_shape_of_no_usable_entry_is_reported_as_none() {
        // `0` with a null result: glibc's spelling of "no such user".
        assert_eq!(lookup(&mut |_, _, _| 0), AccountLookup::NoEntry);
        // A result with no `pw_dir`.
        assert_eq!(
            lookup(&mut |pwd, _, result| {
                *result = pwd;
                0
            }),
            AccountLookup::NoEntry
        );
        // A call that left a `pw_dir` behind but reported no entry (`result` stays null): the
        // pointer is not trusted without the result.
        assert_eq!(
            lookup(&mut |pwd, buf, _result| {
                fill_entry("/home/scribbled", pwd, buf, &mut std::ptr::null_mut());
                0
            }),
            AccountLookup::NoEntry
        );
        // The error-code spellings of "no such user".
        for errno in [libc::ENOENT, libc::ESRCH] {
            assert_eq!(
                lookup(&mut |_, _, _| errno),
                AccountLookup::NoEntry,
                "errno {errno}"
            );
        }
        // An entry whose home is not an absolute path is not a home directory.
        for dir in ["", "rel", "."] {
            assert_eq!(
                lookup(&mut |pwd, buf, result| fill_entry(dir, pwd, buf, result)),
                AccountLookup::NoEntry,
                "pw_dir {dir:?}"
            );
        }
        // An error that says nothing about the entry is a failure, not a verdict.
        assert_eq!(lookup(&mut |_, _, _| libc::EIO), AccountLookup::Failed);
    }

    /// A clock a test moves by hand: `now()` is a fixed instant plus whatever was advanced.
    struct FakeClock {
        base: std::time::Instant,
        offset: std::cell::Cell<std::time::Duration>,
    }

    impl FakeClock {
        fn new() -> Self {
            Self {
                base: std::time::Instant::now(),
                offset: std::cell::Cell::new(std::time::Duration::ZERO),
            }
        }
        fn advance(&self, by: std::time::Duration) {
            self.offset.set(self.offset.get() + by);
        }
        fn now(&self) -> std::time::Instant {
            self.base + self.offset.get()
        }
    }

    #[test]
    fn a_found_answer_is_looked_up_once_and_kept() {
        let cache = AccountCache::new();
        let clock = FakeClock::new();
        let mut lookups = 0;
        for _ in 0..3 {
            let got = cached_account_home(
                &cache,
                || clock.now(),
                || {
                    lookups += 1;
                    AccountLookup::Found(PathBuf::from("/home/fake"))
                },
            );
            assert_eq!(got, Some(PathBuf::from("/home/fake")));
            // A success is kept for the life of the process, however much time passes.
            clock.advance(std::time::Duration::from_secs(86_400));
        }
        assert_eq!(
            lookups, 1,
            "the cache must answer the second and third call"
        );
    }

    #[test]
    fn a_definitive_no_entry_is_kept_too() {
        let cache = AccountCache::new();
        let clock = FakeClock::new();
        let mut lookups = 0;
        for _ in 0..3 {
            let got = cached_account_home(
                &cache,
                || clock.now(),
                || {
                    lookups += 1;
                    AccountLookup::NoEntry
                },
            );
            assert_eq!(got, None);
            clock.advance(PASSWD_FAILURE_TTL * 10);
        }
        assert_eq!(lookups, 1);
    }

    #[test]
    fn a_failed_lookup_is_not_repeated_before_the_ttl() {
        // An NSS outage (LDAP, sssd: EAGAIN or EIO) must not turn every tool call into another
        // blocking `getpwuid_r` on a tokio worker.
        let cache = AccountCache::new();
        let clock = FakeClock::new();
        let mut lookups = 0;
        let mut ask = |clock: &FakeClock| {
            cached_account_home(
                &cache,
                || clock.now(),
                || {
                    lookups += 1;
                    AccountLookup::Failed
                },
            )
        };
        assert_eq!(ask(&clock), None);
        assert_eq!(ask(&clock), None);
        clock.advance(PASSWD_FAILURE_TTL - std::time::Duration::from_nanos(1));
        assert_eq!(ask(&clock), None, "one tick before the window ends");
        assert_eq!(
            lookups, 1,
            "the failure must be remembered for the whole window"
        );
    }

    #[test]
    fn a_failed_lookup_is_retried_after_the_ttl_and_a_success_then_sticks() {
        let cache = AccountCache::new();
        let clock = FakeClock::new();
        let mut lookups = 0;
        let mut outcomes = vec![
            AccountLookup::Found(PathBuf::from("/home/fake")),
            AccountLookup::Failed,
        ];
        let mut ask = |clock: &FakeClock| {
            cached_account_home(
                &cache,
                || clock.now(),
                || {
                    lookups += 1;
                    outcomes.pop().expect("no more lookups expected")
                },
            )
        };
        assert_eq!(ask(&clock), None, "first lookup fails");
        clock.advance(PASSWD_FAILURE_TTL);
        assert_eq!(
            ask(&clock),
            Some(PathBuf::from("/home/fake")),
            "exactly at the TTL the lookup is tried again"
        );
        clock.advance(PASSWD_FAILURE_TTL * 100);
        assert_eq!(ask(&clock), Some(PathBuf::from("/home/fake")));
        assert_eq!(lookups, 2, "a success is kept; no third lookup");
    }

    #[test]
    fn each_failure_restarts_the_window() {
        let cache = AccountCache::new();
        let clock = FakeClock::new();
        let mut lookups = 0;
        let mut ask = |clock: &FakeClock| {
            cached_account_home(
                &cache,
                || clock.now(),
                || {
                    lookups += 1;
                    AccountLookup::Failed
                },
            )
        };
        ask(&clock);
        clock.advance(PASSWD_FAILURE_TTL);
        ask(&clock); // second failure, at t = TTL
        clock.advance(PASSWD_FAILURE_TTL - std::time::Duration::from_nanos(1));
        ask(&clock); // still inside the window that began at the SECOND failure
        assert_eq!(lookups, 2, "the window is measured from the latest failure");
    }

    #[test]
    fn concurrent_first_callers_make_one_lookup_between_them() {
        // Thread A is inside the (slow) lookup, holding the cache's lock; thread B arrives meanwhile.
        // B must wait and then be answered from the cache, not run a second NSS call.
        let cache = std::sync::Arc::new(AccountCache::new());
        let lookups = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let a = {
            let (cache, lookups) = (cache.clone(), lookups.clone());
            std::thread::spawn(move || {
                cached_account_home(&cache, std::time::Instant::now, || {
                    lookups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    started_tx.send(()).unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    AccountLookup::Found(PathBuf::from("/home/fake"))
                })
            })
        };
        started_rx.recv().unwrap(); // A holds the lock now
        let b = cached_account_home(&cache, std::time::Instant::now, || {
            lookups.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            AccountLookup::Found(PathBuf::from("/home/other"))
        });
        assert_eq!(a.join().unwrap(), Some(PathBuf::from("/home/fake")));
        assert_eq!(
            b,
            Some(PathBuf::from("/home/fake")),
            "B is answered from A's result"
        );
        assert_eq!(lookups.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn a_kept_answer_is_served_without_taking_the_lock() {
        // The hot path (every tool call after the first) must not queue behind a lookup in flight.
        let cache = std::sync::Arc::new(AccountCache::new());
        cached_account_home(&cache, std::time::Instant::now, || {
            AccountLookup::Found(PathBuf::from("/home/fake"))
        });
        let _lookup_in_flight = cache.failed_at.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = cache.clone();
        std::thread::spawn(move || {
            let got =
                cached_account_home(&reader, std::time::Instant::now, || AccountLookup::Failed);
            let _ = tx.send(got);
        });
        let got = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("a kept answer must not wait for the lock");
        assert_eq!(got, Some(PathBuf::from("/home/fake")));
    }

    /// A pid that is definitely not running — spawned, then reaped.
    ///
    /// Deliberately NOT a magic constant. `0` and any value ≥ `2^31` are both
    /// "dead" for reasons that have nothing to do with liveness (see
    /// `addressable_pid`), so a constant would let a `process_alive` that never
    /// calls `kill` at all pass this file's liveness tests.
    fn a_reaped_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn liveness_is_true_for_this_process_and_false_for_a_reaped_one() {
        assert!(process_alive(std::process::id()));
        assert!(!process_alive(a_reaped_pid()));
    }

    /// The values where `kill`'s addressing mode changes, which a `pid as i32`
    /// cast reaches silently.
    ///
    /// **Every row here was `true` before 2026-09-05**, which is the whole
    /// point: each one makes a nonexistent process report as alive, so any
    /// stale-holder cleanup keyed on liveness never fires. `u32::MAX` casts to
    /// `-1` (every signalable process); `0` addresses the caller's own process
    /// group, so it is true for any live caller; `0x8000_0000` is the first
    /// value that wraps negative.
    ///
    /// Pair this with the row above: without a genuinely reaped pid there, a
    /// `process_alive` hard-wired to `false` would satisfy this test.
    #[test]
    fn a_pid_that_does_not_address_one_process_is_not_alive() {
        for pid in [0u32, 0x8000_0000, u32::MAX] {
            assert!(
                !process_alive(pid),
                "process_alive({pid}) must be false — it does not name one process"
            );
        }
    }

    /// The same boundary on the signalling path, where the consequence is not a
    /// wrong answer but a wrong *action*: `kill(-1, SIGTERM)` terminates every
    /// process the user owns.
    #[test]
    fn terminating_a_pid_that_does_not_address_one_process_is_refused() {
        for pid in [0u32, 0x8000_0000, u32::MAX] {
            let err = terminate_process(pid).expect_err(
                "must refuse: casting this would signal a process GROUP, not one process",
            );
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "pid {pid}");
        }
    }

    #[test]
    fn temp_dir_exists() {
        assert!(temp_dir().exists());
    }

    #[test]
    fn shell_command_uses_sh() {
        let cmd = shell_command_configured("echo hello");
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program().to_str().unwrap(), "sh");
        let args: Vec<&str> = std_cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(args, vec!["-c", "echo hello"]);
    }

    #[test]
    fn shell_tokenize_splits_correctly() {
        let tokens = shell_tokenize("echo 'hello world'").unwrap();
        assert_eq!(tokens, vec!["echo", "hello world"]);
    }

    #[test]
    fn lsp_binary_name_unchanged() {
        assert_eq!(lsp_binary_name("rust-analyzer"), "rust-analyzer");
    }
}
