#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::testing::runner)]
#![reexport_test_harness_main = "test_main"]
#![cfg_attr(test, allow(dead_code, unused_imports))]

mod framebuffer;
mod panic;
mod serial;
mod testing;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};

#[used]
#[unsafe(link_section = ".limine_requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests_start")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".limine_requests_end")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    serial::init();
    println!("sickos v{}", env!("CARGO_PKG_VERSION"));
    assert!(BASE_REVISION.is_supported(), "limine base revision unsupported");
    
    #[cfg(test)]
    test_main();

    match FRAMEBUFFER_REQUEST
        .get_response()
        .and_then(|r| r.framebuffers().next())
    {
        Some(fb) => {
            println!(
                "framebuffer: {}x{}, {} bpp, pitch {}",
                fb.width(),
                fb.height(),
                fb.bpp(),
                fb.pitch()
            );
            framebuffer::splash(&fb);
        }
        None => println!("framebuffer: none provided"),
    }

    println!("boot ok; halting");
    hcf();
}

/// Halt forever.
pub fn hcf() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(test)]
mod tests {
    // shadows the built-in #[test] (libtest-only) with our in-QEMU version
    use sickos_macros::test;

    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn serial_works() {
        crate::println!("printing from a test");
    }
}
