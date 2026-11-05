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

> **Note:** the kernel only *runs* on `x86_64-unknown-none` — the aliases
> above pass the target for you. Host builds exist purely so IDE project sync
> (`cargo check --workspace`) can analyze the crate: on the host the kernel
> links std, and running it (e.g. a plain `cargo test -p kernel`, what an IDE
> test runner does by default) just prints a pointer to `cargo ost` and exits.
> In RustRover, use Cargo run configurations whose *Command* is `os` / `ost`.

## Agentic

This repository enables Agentic at project scope in all three harnesses:

- Claude Code: `.claude/settings.json` enables `agentic@corebit-instruments`.
- Codex: `.codex/config.toml` enables `agentic@corebit-instruments`.
- OpenCode: `opencode.json` enables `agentic`.

Install the CLI and plugin using the
[Agentic setup instructions](https://github.com/corebit-instruments/agentic#readme),
then start a session in this repository (trusted in Codex). `agentic doctor`
checks the local installation and activation for every harness. `CLAUDE.md`
imports the shared guidance from `AGENTS.md`.

`agentic.toml` is the source of truth for formatting, host and bare-metal
Clippy checks, host tests, and the in-QEMU kernel tests. For unattended
validation in PowerShell:

```powershell
$env:SICKOS_HEADLESS = "1"
agentic local validate
```

`agentic local commit` formats staged Rust changes and runs the commit checks;
`agentic remote pr` runs the full validation before publishing. CI runs the
same declared checks on Windows with headless QEMU. Because the Agentic source
repository is private, CI reads the command arrays directly from `agentic.toml`
without requiring credentials to install the CLI.

Engineering Tasks use the existing
[SickOS Roadmap](https://github.com/users/cmilatinov/projects/3) Project.
See [AGENTS.md](AGENTS.md) for repository guidance.

## Debugging

QEMU has a built-in GDB stub; the kernel is a debug-build ELF statically
linked at its runtime addresses, so source-level debugging just works once a
debugger attaches.

Two entry points (both leave the CPU frozen at reset until you attach):

- `cargo osd` — boots and blocks in the terminal; attach with
  `target\xtask\debug\gdbk.exe` + `target remote localhost:1234`.
- `cargo os -- --gdb-detach` — same, but backgrounds QEMU and returns
  immediately. Serial goes to `target/serial.log` (`Get-Content -Wait
  target\serial.log` to follow it), QEMU diagnostics to `target/qemu.log`.
  This is the mode the RustRover setup below uses. If a stale QEMU is still
  holding port 1234, xtask says so instead of failing silently.

### Debugger: the gdbk wrapper

A Windows-hosted gdb assumes a remote target is a Windows process and probes
it with Windows-only packets (`qGetTIBAddr`), which QEMU's stub rejects,
disconnecting the session. The fix is one flag (`-iex "set osabi none"`) —
but RustRover's Remote Debug config only takes a debugger *path*, no flags.

`tools/gdbk` is that flag in executable form: a tiny wrapper that finds a
gdb (RustRover's own bundled one by default, MSYS2 gdb-multiarch as
fallback, `SICKOS_GDB` env to override), injects the fix, and forwards
everything else untouched. No extra installs, no `.gdbinit` files.

The wrapper is (re)built automatically by `cargo os -- --gdb-detach`, landing
at `target\xtask\debug\gdbk.exe` — run that once before configuring the IDE
so the file exists.

### RustRover: one-click debug

1. **Run → Edit Configurations → + → Remote Debug**, name it e.g.
   `kernel (QEMU)`:
   - Debugger: **Custom GDB binary** → `<repo>\target\xtask\debug\gdbk.exe`
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


## Layout

- `kernel/` — the kernel (target `x86_64-unknown-none`, booted via the Limine protocol)
- `tools/xtask/` — host-side runner: stages the UEFI boot directory, launches QEMU
- `tools/gdbk/` — debugger wrapper: gdb-multiarch preconfigured for bare-metal remote
- `boot/limine.conf` — bootloader config
- `crates/` — (soon) pure-logic `no_std` crates unit-tested on the host
