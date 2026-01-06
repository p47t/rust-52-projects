#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![allow(warnings)]

mod vga_buffer;

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");
    println!("Here are some numbers: {} {}", 42, 1.337);

    panic!("Some panic message");
}
