use crate::println;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!();
    println!("!!! KERNEL PANIC !!!");
    println!("{info}");

    #[cfg(test)]
    crate::testing::exit_qemu(crate::testing::ExitCode::Failure);

    #[cfg(not(test))]
    crate::hcf()
}
