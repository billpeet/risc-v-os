#![no_main]
#![no_std]
#![feature(
    panic_info_message,
    global_asm,
    asm,
    llvm_asm,
    alloc_error_handler,
    custom_test_frameworks,
    alloc_prelude
)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

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

extern "C" {
    pub fn switch_to_user(frame: usize) -> !;
}

// ///////////////////////////////////
// / LANGUAGE STRUCTURES / FUNCTIONS
// ///////////////////////////////////
#[no_mangle]
extern "C" fn eh_personality() {}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    print!("Aborting: ");
    if let Some(_p) = info.location() {
        println!(
            "line {}, file {}: {}",
            _p.line(),
            _p.file(),
            info.message().unwrap()
        );
    } else {
        println!("no information available.");
    }
    abort();
}

#[no_mangle]
extern "C" fn abort() -> ! {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

// Runs in machine mode
#[no_mangle]
extern "C" fn kinit() -> ! {
    uart::Uart::new(0x1000_0000).init();
    println!("Welcome to PeetOS");
    page::init();
    kmem::init();
    process::init();
    plic::set_threshold(0);
    // Enable PLIC interrupts
    // VIRTIO = 1 -> 8
    // UART0 = 10
    for i in 1..=10 {
        plic::enable(i);
        plic::set_priority(i, 1);
    }
    virtio::probe();

    console::init();

    // let mut sh = shell::Shell::new();
    // sh.shell();
    test::init_processes();

    // Schedule first process and switch
    trap::schedule_scheduler();
    scheduler::context_switch();
}

#[no_mangle]
extern "C" fn kinit_hart(hartid: usize) {
    // All non-0 harts initialize here.
    unsafe {
        // We have to store the kernel's table. The tables will be moved
        // back and forth between the kernel's table and user
        // applicatons' tables.
        cpu::mscratch_write((&mut cpu::KERNEL_TRAP_FRAME[hartid] as *mut cpu::TrapFrame) as usize);
        // Copy the same mscratch over to the supervisor version of the
        // same register.
        cpu::sscratch_write(cpu::mscratch_read());
        cpu::KERNEL_TRAP_FRAME[hartid].hartid = hartid;
        // We can't do the following until zalloc() is locked, but we
        // don't have locks, yet :( cpu::KERNEL_TRAP_FRAME[hartid].satp
        // = cpu::KERNEL_TRAP_FRAME[0].satp;
        // cpu::KERNEL_TRAP_FRAME[hartid].trap_stack = page::zalloc(1);
    }
}

// ///////////////////////////////////
// / RUST MODULES
// ///////////////////////////////////

pub mod assembly;
pub mod block;
pub mod buffer;
pub mod console;
pub mod cpu;
pub mod fs;
pub mod kmem;
pub mod lock;
pub mod mmu;
pub mod page;
pub mod plic;
pub mod process;
pub mod random;
pub mod scheduler;
pub mod shell;
pub mod syscall;
pub mod test;
pub mod trap;
pub mod uart;
pub mod virtio;

// ///////////////////////////////////
// / TESTS
// ///////////////////////////////////
#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}

#[test_case]
fn some_test() {
    println!("some test...");
    assert_eq!(1, 1);
    println!("[ok]");
}
