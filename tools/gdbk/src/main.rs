//! gdb wrapper for kernel debugging. Launches gdb with `set osabi none`
//! injected via -iex, forwarding all other args and stdio untouched
//! (RustRover talks MI through us transparently).
//!
//! Why: a Windows-hosted gdb defaults to treating remote targets as Windows
//! processes and probes them with Windows-only packets (qGetTIBAddr), which
//! QEMU's stub rejects, killing the session. RustRover's Remote Debug config
//! has no flags field — only a binary path — so this wrapper is the flag.
//! RustRover's own bundled gdb handles our bare-metal ELF fine once the OS
//! ABI is corrected, so by default no extra gdb needs to be installed.
//!
//! Env: SICKOS_GDB overrides the gdb executable path.

use std::path::PathBuf;
use std::process::{Command, exit};

fn find_gdb() -> PathBuf {
    if let Some(p) = std::env::var_os("SICKOS_GDB") {
        return p.into();
    }
    let mut candidates = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates
            .push(PathBuf::from(local).join(r"Programs\RustRover\bin\gdb\win\x64\bin\gdb.exe"));
    }
    candidates.push(PathBuf::from(r"C:\msys64\mingw64\bin\gdb-multiarch.exe"));
    match candidates.into_iter().find(|c| c.exists()) {
        Some(gdb) => gdb,
        None => {
            eprintln!("[gdbk] no gdb found — set SICKOS_GDB to a gdb executable");
            eprintln!("[gdbk] (RustRover's bundled gdb works, as does MSYS2 gdb-multiarch)");
            exit(1);
        }
    }
}

fn main() {
    let gdb = find_gdb();
    let status = Command::new(&gdb)
        .arg("-iex")
        .arg("set osabi none")
        .args(std::env::args_os().skip(1))
        .status()
        .unwrap_or_else(|e| {
            eprintln!("[gdbk] failed to launch {}: {e}", gdb.display());
            exit(1);
        });
    exit(status.code().unwrap_or(1));
}
