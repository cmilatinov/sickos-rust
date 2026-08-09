# sickos-rust

A minimal x86_64 OS in Rust. See [PLAN.md](PLAN.md) for the roadmap.

## Prerequisites

- Rust via rustup (the pinned nightly + target auto-install from `rust-toolchain.toml`)
- QEMU (`winget install SoftwareFreedomConservancy.QEMU`)

## Commands

| Command | What it does |
|---|---|
| `cargo os` | Build the kernel and boot it in QEMU |
| `cargo ost` | Run the in-QEMU test suite (exits with pass/fail) |
| `cargo osd` | Boot frozen with the gdb stub on `localhost:1234`, wait for a debugger |

Serial output (the kernel log) appears on the terminal. The first run
downloads the Limine bootloader into `target/` (cached afterwards).

Env knobs: `SICKOS_HEADLESS=1` runs QEMU without a window; `QEMU_DIR` points
at a non-standard QEMU install.

> **Note:** the kernel only builds for `x86_64-unknown-none` — the aliases
> above pass the target for you. A plain `cargo test -p kernel` (what an IDE
> test runner does by default) targets the host and fails with a build error
> saying so. In RustRover, create Cargo run configurations whose *Command* is
> `os` / `ost` instead of using the default test runner.

## Debugging

QEMU has a built-in GDB stub; the kernel is a debug-build ELF statically
linked at its runtime addresses, so source-level debugging just works once a
debugger attaches.

Two entry points (both leave the CPU frozen at reset until you attach):

- `cargo osd` — boots and blocks in the terminal; attach any GDB with
  `target remote localhost:1234`.
- `cargo os -- --gdb-detach` — same, but backgrounds QEMU and returns
  immediately. Serial goes to `target/serial.log` (`Get-Content -Wait
  target\serial.log` to follow it), QEMU diagnostics to `target/qemu.log`.
  This is the mode the RustRover setup below uses. If a stale QEMU is still
  holding port 1234, xtask says so instead of failing silently.

### RustRover: one-click debug

1. **Run → Edit Configurations → + → Remote Debug**, name it e.g.
   `kernel (QEMU)`:
   - Debugger: **Bundled GDB**
   - `'target remote' args`: `localhost:1234`
   - Symbol file: `<repo>\target\x86_64-unknown-none\debug\kernel`
   - Sysroot / path mappings: leave empty
2. In the same dialog, **Before launch → + → Run External Tool → +**:
   - Program: `cargo` — Arguments: `os -- --gdb-detach` — Working directory:
     `$ProjectFileDir$`
3. Hit **Debug**: it builds the kernel, boots QEMU frozen, and attaches.

**First breakpoint gotcha:** at attach time the CPU is parked in the firmware
— the kernel isn't loaded and its (higher-half) addresses aren't mapped yet,
so IDE line breakpoints can't be inserted yet. In the debugger's **GDB**
console tab run:

```
hbreak kmain
continue
```

Hardware breakpoints (`hbreak`, max 4) don't need the target memory to be
mapped. Once it stops in `kmain`, paging is live and normal IDE line
breakpoints work everywhere — set them and resume as usual.

If the bundled GDB refuses the ELF or the remote target, install a
multiarch/x86_64-elf GDB (e.g. via MSYS2) and select it as a custom debugger
toolchain in RustRover.

## Layout

- `kernel/` — the kernel (target `x86_64-unknown-none`, booted via the Limine protocol)
- `tools/xtask/` — host-side runner: stages the UEFI boot directory, launches QEMU
- `boot/limine.conf` — bootloader config
- `crates/` — (soon) pure-logic `no_std` crates unit-tested on the host
