//! In-QEMU test harness. `cargo ost` builds the kernel with these tests
//! compiled in; the guest reports pass/fail through the `isa-debug-exit`
//! device, which makes QEMU exit with `(code << 1) | 1`.

use crate::{print, println};

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum ExitCode {
    Success = 0x10, // QEMU exit code 33
    Failure = 0x11, // QEMU exit code 35
}

pub fn exit_qemu(code: ExitCode) -> ! {
    let mut port = x86_64::instructions::port::Port::new(0xf4);
    unsafe { port.write(code as u32) };
    crate::hcf()
}

pub trait TestCase {
    fn run(&self);
}

impl<T: Fn()> TestCase for T {
    fn run(&self) {
        print!("test {} ... ", core::any::type_name::<T>());
        self();
        println!("ok");
    }
}

#[allow(unreachable_code)] // host builds bail before the real harness below
pub fn runner(tests: &[&dyn TestCase]) {
    // a host build of this harness can compile (for IDE analysis) but must
    // never run — the first port I/O would crash the process
    #[cfg(not(target_os = "none"))]
    {
        let _ = tests; // the real harness below is unreachable on the host
        eprintln!("kernel tests only run inside QEMU: use `cargo ost`");
        std::process::exit(2);
    }

    println!("running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    println!("all tests passed");
    exit_qemu(ExitCode::Success)
}
