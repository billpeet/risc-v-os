#![no_std]
#![feature(panic_info_message,asm,alloc_error_handler)]

// ///////////////////////////////////
// / RUST MACROS
// ///////////////////////////////////
#[macro_export]
macro_rules! print
{
    ($($args:tt)+) => ({
        use core::fmt::Write;
        let _ = write!(crate::uart::Uart::new(0x1000_0000), $($args)+);
    });
}

#[macro_export]
macro_rules! println
{
    () => ({
        print!("\r\n")
    });
    ($fmt:expr) => ({
        print!(concat!($fmt, "\r\n"))
    });
    ($fmt:expr, $($args:tt)+) => ({
        print!(concat!($fmt, "\r\n"), $($args)+)
    });
}

extern "C" {
    pub static HEAP_START: usize;
    pub static HEAP_SIZE: usize;
    pub static TEXT_START: usize;
    pub static TEXT_END: usize;
    pub static RODATA_START: usize;
    pub static RODATA_END: usize;
    pub static DATA_START: usize;
    pub static DATA_END: usize;
    pub static BSS_START: usize;
    pub static BSS_END: usize;
    pub static KERNEL_STACK_START: usize;
    pub static KERNEL_STACK_END: usize;
}

// ///////////////////////////////////
// / LANGUAGE STRUCTURES / FUNCTIONS
// ///////////////////////////////////
#[no_mangle]
extern "C" fn eh_personality() {}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> !{
    print!("Aborting: ");
	if let Some(_p) = info.location() {
		println!(
            "line {}, file {}: {}",
            _p.line(),
            _p.file(),
            info.message().unwrap()
		);
	}
    else {
        println!("no information available.");
    }
    abort();
}

#[no_mangle]
extern "C"
fn abort() -> ! {
    loop {
        unsafe {
            asm!("wfi"::::"volatile");
        }
    }
}

// Runs in machine mode
#[no_mangle]
extern "C"
fn kinit() {
    uart::Uart::new(0x1000_0000).init();
    page::init();
    kmem::init();
    mmu::map_kernel();
}

// Entry point, in supervisor mode
#[no_mangle]
extern "C"
fn kmain() {
    println!("Welcome to PeetOS");
    //page::paging_tests();
    //kmem::kmem_tests();
    kmem::global_alloc_tests();

    // Shell
    let mut my_shell = shell::Shell::new();
    my_shell.shell();
}

// ///////////////////////////////////
// / RUST MODULES
// ///////////////////////////////////

pub mod uart;
pub mod page;
pub mod shell;
pub mod mmu;
pub mod kmem;