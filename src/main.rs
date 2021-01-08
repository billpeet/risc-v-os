#![no_main]
#![no_std]
#![feature(panic_info_message,global_asm,asm,llvm_asm,alloc_error_handler,custom_test_frameworks,alloc_prelude)]
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
    fn switch_to_user(frame: usize, epc: usize, satp: usize) -> !;
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
            asm!("wfi");
        }
    }
}

// Runs in machine mode
#[no_mangle]
extern "C"
fn kinit() -> ! {
    uart::Uart::new(0x1000_0000).init();
    println!("Welcome to PeetOS");
    page::init();
    kmem::init();
    process::init();
    println!("Setting up interrupts and PLIC...");
    plic::set_threshold(0);
    // Enable PLIC interrupts
    // VIRTIO = 1 -> 8
    // UART0 = 10
    for i in 1..=10 {
        plic::enable(i);
        plic::set_priority(i, 1);
    }
    virtio::probe();

    println!("testing block driver");
    let buffer = kmem::kmalloc(1024);
    unsafe { buffer.write_volatile(1) }
    block::read(3, buffer, 512, 1024);
    let mut i = 0;
    loop {
        if i > 100_000_000 {
            break;
        }
        i += 1;
    }
    unsafe {
		print!("  ");
		for i in 0..16 {
			print!("{:02x} ", buffer.add(i).read());
		}
		println!();
		print!("  ");
		for i in 0..16 {
			print!("{:02x} ", buffer.add(16+i).read());
		}
		println!();
		print!("  ");
		for i in 0..16 {
			print!("{:02x} ", buffer.add(32+i).read());
		}
		println!();
		print!("  ");
		for i in 0..16 {
			print!("{:02x} ", buffer.add(48+i).read());
		}
		println!();
		buffer.add(0).write(0xaa);
		buffer.add(1).write(0xbb);
		buffer.add(2).write(0x7a);
    }
    block::write(3, buffer, 512, 0);
    kmem::kfree(buffer);

    // let mut sh = shell::Shell::new();
    // sh.shell();

    // Start timer
    unsafe {
        let mtimecmp = 0x0200_4000 as *mut u64;
        let mtime = 0x0200_bff8 as *const u64;
        // The frequency given by QEMU is 10_000_000 Hz, so this sets
        // the next interrupt to fire one second from now.
        mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000);
    }

    // Schedule first process and switch
    let (frame, mepc, satp) = scheduler::schedule();
    unsafe {
        switch_to_user(frame, mepc, satp);
    }
}

#[no_mangle]
extern "C"
fn kinit_hart(hartid: usize) {
	// All non-0 harts initialize here.
	unsafe {
		// We have to store the kernel's table. The tables will be moved
		// back and forth between the kernel's table and user
		// applicatons' tables.
		cpu::mscratch_write(
            (&mut cpu::KERNEL_TRAP_FRAME[hartid]
                as *mut cpu::TrapFrame)
            as usize,
		);
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
pub mod uart;
pub mod page;
pub mod shell;
pub mod mmu;
pub mod kmem;
pub mod trap;
pub mod cpu;
pub mod plic;
pub mod process;
pub mod syscall;
pub mod scheduler;
pub mod virtio;
pub mod block;
pub mod random;

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