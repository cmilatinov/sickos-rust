# sickos-rust — Plan

A minimal x86_64 OS in Rust. Handcode the logic, outsource the fiddly bit-encodings,
and invest in debuggability *before* every feature so debugging sessions rarely happen.

## Decisions (locked)

| Decision | Choice | Why |
|---|---|---|
| Arch | x86_64, QEMU (q35) as primary target | Best tooling, best docs |
| Boot | **Limine** boot protocol | Long mode, memory map, HHDM, and framebuffer handed to us; skips the most debugging-heavy phase entirely |
| Crate policy | **Pragmatic** | Crates for error-prone *encodings* (`x86_64`, `uart_16550`, `pc-keyboard`, `bitflags`, `spin`, `limine`); all *logic* handcoded (allocators, mapper policy, scheduler, VFS, drivers, shell) |
| GPU | **virtio-gpu** | Modern, first-class in QEMU; virtqueue infra is reused by virtio-blk/input |
| SMP | Single core until late | Concurrency bugs are the worst to debug; design per-CPU from day one, boot APs only after everything is stable |
| Userspace | Out of scope for "minimal" | Kernel-mode shell instead; ring 3 + syscalls + ELF loader is a later, optional phase |

## Repo layout

```
sickos-rust/
├── Cargo.toml          # workspace
├── kernel/             # the kernel binary (target: x86_64-unknown-none)
├── crates/             # pure-logic no_std crates that ALSO build on the host
│   ├── vfs/            #   → unit-tested with plain `cargo test`, no VM involved
│   └── ...             #   (fs drivers, path parsing, allocator logic as they appear)
└── tools/
    └── xtask/          # host-side builder: makes the boot image, launches QEMU
```

Windows-friendly build: no xorriso/ISO, no disk image at all. `xtask` stages a
directory (`target/esp/`) with `EFI/BOOT/BOOTX64.EFI` (Limine), `limine.conf`, and the
kernel ELF, and QEMU serves it as a virtual FAT drive (`-drive file=fat:rw:...`) booted
via the bundled edk2 UEFI firmware. `cargo os` = build → stage → QEMU; `cargo ost` runs
the in-QEMU tests.

## The anti-debugging playbook (applies to every phase)

1. **Serial output before anything else.** `println!`/`log!` over COM1 from the first
   instruction we control. Every subsystem logs its init and its failures.
2. **Verbose exception handlers on day one.** All 32 CPU exceptions print vector, error
   code, RIP, CR2, register dump — *before* any feature work starts. A page fault must
   never look like a hang.
3. **Double-fault handler on a known-good IST stack.** Turns would-be silent
   triple-fault reboot loops into a readable panic message. Non-negotiable, done early.
4. **QEMU flags that preserve the crime scene:** `-no-reboot -no-shutdown -d int,guest_errors`
   — a triple fault freezes with state on screen instead of reboot-looping.
5. **Test where debugging is cheap.** Pure logic (VFS, path resolution, fs drivers,
   allocator logic) lives in `crates/` and runs under `cargo test` on the host with
   full std debugging. In-kernel integration tests boot QEMU and report pass/fail via
   `isa-debug-exit`.
6. **Tiny unsafe surface.** `unsafe` confined to `kernel/src/arch/` and MMIO wrappers;
   everything above is safe Rust. Liberal `debug_assert!`, poisoned freed memory
   (`0xDEAD_DEAD`), stack canaries on kernel stacks.
7. **Panic = maximum information.** Panic handler prints location + message + RIP chain
   (frame pointers forced on); an `addr2line` script in `xtask` symbolizes offline.
8. **GDB escape hatch wired in** (`cargo osd` opens QEMU's gdb stub; RustRover
   attach recipe in the README) for the rare cases the above misses.

## Phases

### Phase 0 — Scaffold & debuggability
Boots via Limine, prints over serial and to the Limine framebuffer (bitmap font
console), panics informatively, and has the test harness wired up.
- Pinned nightly toolchain (`rust-toolchain.toml`), `x86_64-unknown-none`
- `xtask`: disk image builder + QEMU runner (+ OVMF), one command
- Serial driver, `log` facade with levels, framebuffer text console
- Panic handler, custom test runner + `isa-debug-exit`
- **Done when:** `cargo run` shows "hello" on serial + screen; `cargo test` runs a
  trivial in-QEMU test; a deliberate `panic!` prints file/line.

### Phase 1 — CPU tables & interrupts
- GDT + TSS with IST stacks; double-fault handler first
- IDT with verbose handlers for all exceptions
- Legacy PIC masked; LAPIC + IOAPIC; LAPIC timer (calibrated via PIT)
- PS/2 keyboard interrupt → input queue
- **Done when:** timer ticks log at a steady rate; keystrokes echo; `int3` and a forced
  page fault both produce clean register dumps instead of hangs.

### Phase 2 — Memory
- Parse Limine memory map → physical frame allocator (bitmap first; buddy later if wanted)
- Virtual memory: mapper built on the HHDM (map/unmap/translate/protect), guard pages
  under kernel stacks
- Kernel heap: handwritten free-list allocator behind `GlobalAlloc` → `alloc` enabled
  (`Box`, `Vec`, `String`)
- Allocator logic lives in `crates/` → fuzzed/unit-tested on host
- **Done when:** heap stress test (in-QEMU) passes; use-after-free of poisoned memory
  is caught by an assert, not by corruption.

### Phase 3 — Scheduling & context switching
- Per-CPU area via GS base (single CPU for now, but the abstraction is per-CPU)
- Kernel threads: the context-switch asm stub (small, isolated, one of the few
  unavoidable asm bits), thread control blocks, run queue
- Preemptive round-robin off the LAPIC timer; `yield`, `sleep(ms)`, block/wake
- Spinlocks with IRQ-disable guards; blocking mutex once the scheduler exists
- **Done when:** N threads print interleaved output; a sleeping thread wakes on time;
  a deliberate deadlock is detected by a lock-timeout assert.

### Phase 4 — Drivers core
- PCIe enumeration (ECAM via ACPI MCFG), device/vendor table, `Driver` trait + registry
- MSI/MSI-X interrupt allocation (skips IOAPIC routing pain for modern devices)
- virtio core: split virtqueues, device negotiation — **shared by gpu/blk/input**
- virtio-blk driver → generic block device layer
- **Done when:** `lspci`-style dump logs the bus; virtio-blk reads/writes sectors of a
  test image verified against the host copy.

### Phase 5 — VFS
- VFS traits (vnode, file ops, mount table), path resolution, open-file table
- ramfs/tmpfs first (pure in-memory — no hardware in the loop while the VFS stabilizes)
- devfs (`/dev/console`, `/dev/null`, `/dev/fb0`)
- FAT32 read-only on virtio-blk (host tools can build fixture images; driver is
  host-unit-tested against image files in `crates/`)
- **Done when:** mount ramfs at `/`, create/read/write/list files; mount FAT32 image
  and read a file created on the host.

### Phase 6 — Shell
- Kernel-mode shell as a kernel thread: line editing, history, on keyboard + console
- Built-ins calling straight into the kernel: `ls cat echo write mkdir mount ps mem
  uptime clear help`
- **Done when:** interactive session navigating ramfs + FAT32 works.

### Phase 7 — virtio-gpu
- Modeset + scanout + transfer (2D), replacing the Limine framebuffer
- Double-buffered console; simple rect/blit primitives
- Optional: virtio-input (mouse), a toy compositor
- **Done when:** console runs on virtio-gpu at native resolution; a demo draws at 60fps.

### Later / optional
Ring 3 + syscalls + ELF loader (userspace shell), SMP bring-up, virtio-net + a tiny
TCP stack, writable FAT32, buddy allocator + slab.

## Dependency graph

Phases 0→1→2→3 are strictly sequential. After 3: Phase 4 (drivers) and Phase 5's
ramfs/VFS-core are independent and can interleave; FAT32 needs virtio-blk (4);
Shell (6) needs VFS + keyboard; virtio-gpu (7) needs virtio core (4) only.
