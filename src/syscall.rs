use crate::cpu::TrapFrame;

pub fn do_syscall(mepc: usize, frame: *mut TrapFrame) -> usize {
    let syscall_number;
    unsafe {
        // Read A0 (x10) for syscall number
        syscall_number = (*frame).regs[10];
    }

    match syscall_number {
        0 => {
            println!("You called the exit system call!");
            mepc + 4
        },
        1 => {
            println!("syscall 1 from 0x{:08x}", mepc);
            mepc + 4
        },
        _ => {
            println!("Unknown syscall number {}", syscall_number);
            mepc + 4
        }
    }
}