//! COM1 serial output — the kernel's primary log sink from the first
//! instruction on. QEMU mirrors it to stdio via `-serial stdio`.

use core::fmt::{self, Write};

use spin::Mutex;
use uart_16550::SerialPort;

static COM1: Mutex<Option<SerialPort>> = Mutex::new(None);

pub fn init() {
    let mut port = unsafe { SerialPort::new(0x3F8) };
    port.init();
    *COM1.lock() = Some(port);
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    // Interrupts are not enabled yet; once they are, this must disable them
    // while holding the lock to stay deadlock-free (Phase 1).
    if let Some(port) = COM1.lock().as_mut() {
        let _ = port.write_fmt(args);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::serial::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
