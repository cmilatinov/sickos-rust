//! Host-side build/run tool. Invoked by cargo as the runner for
//! x86_64-unknown-none binaries (see .cargo/config.toml): it stages a UEFI
//! boot directory (Limine + limine.conf + kernel), then boots it in QEMU
//! using QEMU's virtual-FAT drive — no disk image file needed.
//!
//! Env overrides:
//!   QEMU_DIR=<dir>       directory containing qemu-system-x86_64
//!   SICKOS_HEADLESS=1    run QEMU without a display window

use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, exit};

/// Limine binary release (UEFI executable), pinned to the 9.x line.
const LIMINE_EFI_URL: &str =
    "https://raw.githubusercontent.com/limine-bootloader/limine/v9.x-binary/BOOTX64.EFI";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, elf, flags @ ..] if cmd == "run-elf" => {
            let mut gdb = false;
            let mut detach = false;
            for flag in flags {
                match flag.as_str() {
                    "--gdb" => gdb = true,
                    "--gdb-detach" => (gdb, detach) = (true, true),
                    // anything else is a libtest harness flag or test filter
                    // (IDE test runners append e.g. `--format=json --exact`);
                    // the in-QEMU harness always runs every test — ignore them
                    other => eprintln!("[xtask] ignoring test-harness arg: {other}"),
                }
            }
            run_elf(Path::new(elf), gdb, detach);
        }
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("usage: xtask run-elf <kernel-elf> [--gdb | --gdb-detach]");
    eprintln!("  --gdb         open the gdb stub on localhost:1234, freeze CPU until attach");
    eprintln!("  --gdb-detach  same, but background QEMU and return (serial -> target/serial.log)");
    eprintln!("(normally invoked for you: use `cargo os`, `cargo ost`, or `cargo osd`)");
    exit(2);
}

fn run_elf(elf: &Path, gdb: bool, detach: bool) {
    let root = workspace_root();
    let target = root.join("target");

    // Stage the virtual EFI system partition.
    let esp = target.join("esp");
    let boot_dir = esp.join("EFI").join("BOOT");
    std::fs::create_dir_all(&boot_dir).expect("create ESP directories");
    let limine_efi = ensure_limine(&target);
    std::fs::copy(&limine_efi, boot_dir.join("BOOTX64.EFI")).expect("copy BOOTX64.EFI");
    std::fs::copy(root.join("boot").join("limine.conf"), esp.join("limine.conf"))
        .expect("copy limine.conf");
    std::fs::copy(elf, esp.join("kernel.elf")).expect("copy kernel ELF");

    // UEFI firmware: read-only code flash + a fresh writable vars flash.
    let (qemu, fw_dir) = find_qemu();
    let fw_code = fw_dir.join("edk2-x86_64-code.fd");
    let fw_vars = target.join("ovmf-vars.fd");
    std::fs::copy(fw_dir.join("edk2-i386-vars.fd"), &fw_vars).expect("copy UEFI vars template");

    let mut cmd = Command::new(qemu);
    cmd.args(["-machine", "q35"])
        .args(["-m", "512M"])
        .arg("-drive")
        .arg(format!(
            "if=pflash,format=raw,readonly=on,file={}",
            fw_code.display()
        ))
        .arg("-drive")
        .arg(format!("if=pflash,format=raw,file={}", fw_vars.display()))
        .arg("-drive")
        .arg(format!("format=raw,file=fat:rw:{}", esp.display()))
        // lets the guest report test pass/fail as an exit code
        .args(["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"])
        // a triple fault should exit (visibly), not reboot-loop
        .arg("-no-reboot")
        .args(["-d", "guest_errors"]);
    if gdb {
        // gdb stub on localhost:1234 (-s); freeze the CPU at reset until a
        // debugger attaches and resumes it (-S)
        cmd.args(["-s", "-S"]);
    }
    if detach {
        // xtask returns before QEMU exits, so serial can't share our stdio
        cmd.arg("-serial")
            .arg(format!("file:{}", target.join("serial.log").display()));
    } else {
        cmd.args(["-serial", "stdio"]);
    }
    if env::var_os("SICKOS_HEADLESS").is_some() {
        cmd.args(["-display", "none"]);
    }

    if detach {
        ensure_gdb_port_free();
        let log = File::create(target.join("qemu.log")).expect("create qemu.log");
        cmd.stdin(Stdio::null())
            .stdout(log.try_clone().expect("dup qemu.log handle"))
            .stderr(log);
        let mut child = cmd.spawn().expect("launch qemu-system-x86_64");
        // catch immediate deaths (bad args, locked files) that would otherwise
        // only surface as a debugger failing to attach
        std::thread::sleep(std::time::Duration::from_millis(500));
        if let Ok(Some(status)) = child.try_wait() {
            eprintln!("[xtask] QEMU exited immediately ({status}); qemu.log says:");
            if let Ok(log) = std::fs::read_to_string(target.join("qemu.log")) {
                eprint!("{log}");
            }
            exit(1);
        }
        println!("[xtask] QEMU started detached (pid {}), CPU frozen at reset", child.id());
        println!("[xtask] gdb stub: localhost:1234");
        println!("[xtask] serial:   {}", target.join("serial.log").display());
        return;
    }

    let status = cmd.status().expect("launch qemu-system-x86_64");
    match status.code() {
        Some(33) => exit(0), // isa-debug-exit: ExitCode::Success
        Some(35) => {
            eprintln!("[xtask] guest reported test FAILURE");
            exit(1);
        }
        Some(c) => exit(c),
        None => exit(1),
    }
}

/// A stale detached QEMU would make the new instance die invisibly (its
/// stderr goes to a file — port 1234 and serial.log are both still held).
/// Probe by connecting: a plain bind() test is fooled on Windows, where
/// binding 127.0.0.1 succeeds while another process holds 0.0.0.0.
fn ensure_gdb_port_free() {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 1234));
    if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(250)).is_ok() {
        eprintln!("[xtask] gdb port 1234 is already in use — a previous QEMU is likely still running.");
        eprintln!("[xtask] kill it and retry (PowerShell: Stop-Process -Name qemu-system-x86_64).");
        exit(1);
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask lives at tools/xtask, two levels below the workspace root")
        .to_path_buf()
}

/// Download (once) and cache the Limine UEFI bootloader.
fn ensure_limine(target: &Path) -> PathBuf {
    let dest = target.join("limine-BOOTX64.EFI");
    if dest.exists() {
        return dest;
    }
    eprintln!("[xtask] downloading Limine bootloader...");
    let status = Command::new("curl")
        .arg("-fsSL")
        .arg("-o")
        .arg(&dest)
        .arg(LIMINE_EFI_URL)
        .status()
        .expect("run curl");
    if !status.success() {
        let _ = std::fs::remove_file(&dest);
        eprintln!("[xtask] failed to download {LIMINE_EFI_URL}");
        exit(1);
    }
    dest
}

/// Locate qemu-system-x86_64 and the directory holding its edk2 firmware.
fn find_qemu() -> (PathBuf, PathBuf) {
    let exe_name = if cfg!(windows) {
        "qemu-system-x86_64.exe"
    } else {
        "qemu-system-x86_64"
    };

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = env::var_os("QEMU_DIR") {
        candidates.push(PathBuf::from(dir));
    }
    if cfg!(windows) {
        candidates.push(PathBuf::from(r"C:\Program Files\qemu"));
    }
    let finder = if cfg!(windows) { "where" } else { "which" };
    if let Ok(out) = Command::new(finder).arg("qemu-system-x86_64").output() {
        if out.status.success() {
            if let Some(line) = String::from_utf8_lossy(&out.stdout).lines().next() {
                if let Some(dir) = Path::new(line.trim()).parent() {
                    candidates.push(dir.to_path_buf());
                }
            }
        }
    }

    for dir in candidates {
        let exe = dir.join(exe_name);
        if !exe.exists() {
            continue;
        }
        // firmware lives next to the exe or in <dir>/share depending on platform
        for fw in [dir.join("share"), dir.clone(), dir.join("..").join("share")] {
            if fw.join("edk2-x86_64-code.fd").exists() {
                return (exe, fw);
            }
        }
    }

    eprintln!("[xtask] qemu-system-x86_64 (with edk2 firmware) not found.");
    eprintln!("[xtask] install QEMU or point QEMU_DIR at its install directory.");
    exit(1);
}
