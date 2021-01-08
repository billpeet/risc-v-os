// Process scheduler

use crate::process::{ProcessState, PROCESS_LIST};

pub fn schedule() -> (usize, usize, usize) {
    unsafe {
        if let Some(mut pl) = PROCESS_LIST.take() {
            pl.rotate_left(1);
            let mut frame_addr: usize = 0;
            let mut mepc: usize = 0;
            let mut satp: usize = 0;
            let mut pid: usize = 0;
            if let Some(prc) = pl.front() {
                match prc.get_state() {
                    ProcessState::Running => {
                        frame_addr = prc.get_frame_address();
                        mepc = prc.get_program_counter();
                        satp = prc.get_table_address() >> 12;
                        pid = prc.get_pid() as usize;
                    },
                    ProcessState::Sleeping => {
                        
                    },
                    _ => {},
                }
            }
            println!("Scheduling {}", pid);
            PROCESS_LIST.replace(pl);
            if frame_addr != 0 {
                if satp != 0 {
                    return (frame_addr, mepc, (8 << 60) | (pid << 44) | satp);
                }
                else {
                    return (frame_addr, mepc, 0);
                }
            }
        }
    }

    (0, 0, 0)
}