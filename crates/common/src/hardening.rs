//! Process hardening for security.
//!
//! Applies pre-main security measures to prevent process-level attacks:
//!
//! - **Disable core dumps** (`setrlimit(RLIMIT_CORE, 0)`) — prevents secrets
//!   from leaking into core dump files.
//! - **Prevent ptrace** (Linux: `prctl(PR_SET_DUMPABLE, 0)`, macOS: `ptrace(PT_DENY_ATTACH)`)
//!   — prevents other processes from attaching a debugger.
//! - **Clear dangerous environment variables** (`LD_PRELOAD`, `LD_LIBRARY_PATH`,
//!   `DYLD_*`, etc.) — prevents library injection attacks.
//!
//! These are best-effort: failures are logged but do not abort the process.
//!
//! Ported from OpenAI Codex `process-hardening/src/lib.rs`.

/// Apply all hardening measures.
///
/// Call this early in `main()` before any untrusted input is processed.
pub fn harden_process() {
    clear_dangerous_env_vars();

    #[cfg(target_os = "linux")]
    {
        disable_core_dumps();
        prevent_ptrace_linux();
    }

    #[cfg(target_os = "macos")]
    {
        disable_core_dumps();
        prevent_ptrace_macos();
    }

    // Windows: no equivalent hardening needed for these attack vectors.
}

/// Remove environment variables that could be used for library injection
/// or other process manipulation.
fn clear_dangerous_env_vars() {
    const DANGEROUS_PREFIXES: &[&str] = &[
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_AUDIT",
        "LD_DEBUG",
        "LD_PROFILE",
        "LD_BIND_NOW",
        "LD_TRACE",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "DYLD_FRAMEWORK_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "DYLD_PRINT_LIBRARIES",
    ];

    for var in DANGEROUS_PREFIXES {
        if std::env::var_os(var).is_some() {
            std::env::remove_var(var);
        }
    }
}

/// Disable core dumps on Unix systems.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn disable_core_dumps() {
    // SAFETY: setrlimit is a standard POSIX call. We're setting RLIMIT_CORE
    // to zero which is always safe.
    unsafe {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        let _ = libc::setrlimit(libc::RLIMIT_CORE, &limit);
    }
}

/// Prevent ptrace attachment on Linux.
#[cfg(target_os = "linux")]
fn prevent_ptrace_linux() {
    // SAFETY: prctl with PR_SET_DUMPABLE is safe.
    unsafe {
        let _ = libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
    }
}

/// Prevent ptrace attachment on macOS.
#[cfg(target_os = "macos")]
fn prevent_ptrace_macos() {
    // PT_DENY_ATTACH = 31 on macOS.
    const PT_DENY_ATTACH: libc::c_int = 31;
    // SAFETY: ptrace(PT_DENY_ATTACH, 0, 0, 0) is a standard macOS call.
    unsafe {
        let _ = libc::ptrace(PT_DENY_ATTACH, 0, std::ptr::null_mut::<libc::c_char>(), 0);
    }
}
