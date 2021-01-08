// trap.rs
// Trap routines
use crate::{plic, uart};
use crate::cpu::TrapFrame;
use crate::syscall::do_syscall;
use crate::scheduler::schedule;

extern "C" {
    fn switch_to_user(frame: usize, epc: usize, satp: usize) -> !;
}

#[no_mangle]
extern "C" fn m_trap(epc: usize, tval: usize, cause: usize, hart: usize, _status: usize, frame: &mut TrapFrame) -> usize {
    // async if 64th bit is 1
    let is_async = {
        if cause >> 63 & 1 == 1 {
            true
        } else {
            false
        }
    };

    let cause_num = cause & 0xfff;
    let mut return_pc = epc;
    if is_async {
        // async trap
        match cause_num {
            3 => {
                // Machine software
                println!("Machine software interrupt CPU#{}", hart);
            },
            7 => unsafe {
                // Machine timer
                // Context switch
                let (frame, mepc, satp) = schedule();
                let mtimecmp = 0x0200_4000 as *mut u64;
                let mtime = 0x0200_bff8 as *const u64;
                // Set next interrupt to fire 1 second from now (QEMU frequency is 10,000,000Hz)
                mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000);
                // Switch to process
                switch_to_user(frame, mepc, satp);
            },
            11 => {
                // External interrupt from PLIC
                println!("plic interrupt");
                plic::handle_interrupt();
            },
            _ => {
                panic!("Unhandled async trap CPU#{} -> {}\n", hart, cause_num);
            }
        }
    }
    else {
        // sync trap
        match cause_num {
            2 => {
				// Illegal instruction
                panic!("Illegal instruction CPU#{} -> 0x{:08x}: 0x{:08x}\n", hart, epc, tval);
                loop {}
			},
			8 => {
				// Environment (system) call from User mode
                // println!("E-call from User mode! CPU#{} -> 0x{:08x}", hart, epc);
                return_pc = do_syscall(return_pc, frame);
			},
			9 => {
				// Environment (system) call from Supervisor mode
				println!("E-call from Supervisor mode! CPU#{} -> 0x{:08x}", hart, epc);
				return_pc += 4;
			},
			11 => {
				// Environment (system) call from Machine mode
				panic!("E-call from Machine mode! CPU#{} -> 0x{:08x}\n", hart, epc);
			},
			// Page faults
			12 => {
				// Instruction page fault
				println!("Instruction page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
                loop {}
				return_pc += 4; // jump over current instruction
			},
			13 => {
				// Load page fault
				println!("Load page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
                loop {}
				return_pc += 4; // jump over current instruction
			},
			15 => {
				// Store page fault
				println!("Store page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
                loop {}
				return_pc += 4; // jump over current instruction
			},
			_ => {
				panic!("Unhandled sync trap CPU#{} -> {}\n", hart, cause_num);
			}
        }
    }

    return_pc
}