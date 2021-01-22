// trap.rs
// Trap routines
use crate::{plic, process};
use crate::cpu::{TrapFrame, get_mtime, set_next_minterrupt};
use crate::syscall::do_syscall;
use crate::scheduler::context_switch;

const SCHEDULER_FREQUENCY: u64 = 10_000_000;

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
            7 => {
                // Context switch machine timer
                schedule_scheduler();
                context_switch();
            },
            11 => {
                // External interrupt from PLIC
                // println!("plic interrupt");
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
                println!("Illegal instruction CPU#{}, PID {}, PC 0x{:08x}, MEPC 0x{:08x}\n", hart, (*frame).pid, (*frame).pc, epc);
                process::delete_process((*frame).pid as u16);
                schedule_scheduler();
                context_switch();
            },
            3 => {
                // Breakpoint
                println!("breakpoint\r\n");
                return_pc += 2;

            },
            4 => {
				// Load address misaligned
                println!("Load address misaligned CPU#{}, PID {}, PC 0x{:08x}, MEPC 0x{:08x}\n", hart, (*frame).pid, (*frame).pc, epc);
                process::delete_process((*frame).pid as u16);
                schedule_scheduler();
                context_switch();
            },
            5 => {
				// Load access fault
                println!("Load access fault CPU#{}, PID {}, PC 0x{:08x}, MEPC 0x{:08x}\n", hart, (*frame).pid, (*frame).pc, epc);
                process::delete_process((*frame).pid as u16);
                schedule_scheduler();
                context_switch();
            },
            6 => {
				// Store/AMO address misaligned
                println!("Store/AMO address misaligned CPU#{}, PID {}, PC 0x{:08x}, MEPC 0x{:08x}\n", hart, (*frame).pid, (*frame).pc, epc);
                process::delete_process((*frame).pid as u16);
                schedule_scheduler();
                context_switch();
            },
            7 => {
				// Store/AMO access fault
                println!("Store/AMO access fault CPU#{}, PID {}, PC 0x{:08x}, MEPC 0x{:08x}\n", hart, (*frame).pid, (*frame).pc, epc);
                process::delete_process((*frame).pid as u16);
                schedule_scheduler();
                context_switch();
			},
			8 | 9 | 11 => unsafe {
				// E-call from User mode
                let switch_required = do_syscall(return_pc, frame);
                return_pc += 4;
                if switch_required == true {
                    schedule_scheduler();
                    context_switch();
                }
			},
			// Page faults
			12 => {
				// Instruction page fault
				println!("Instruction page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
                process::delete_process((*frame).pid as u16);
                schedule_scheduler();
                context_switch();
			},
			13 => {
				// Load page fault
				println!("Load page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
                process::delete_process((*frame).pid as u16);
                schedule_scheduler();
                context_switch();
			},
			15 => {
				// Store page fault
				println!("Store page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
                process::delete_process((*frame).pid as u16);
                schedule_scheduler();
                context_switch();
			},
			_ => {
				panic!("Unhandled sync trap CPU#{} -> {}\n", hart, cause_num);
			}
        }
    }

    return_pc
}

pub fn schedule_scheduler() {
    // Set next machine timer interrupt
    let next_time = get_mtime().offset_ticks(SCHEDULER_FREQUENCY);
    set_next_minterrupt(next_time);
}