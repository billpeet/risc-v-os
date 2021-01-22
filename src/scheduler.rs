// Process scheduler
use crate::process::{ProcessState, PROCESS_LIST, PROCESS_LIST_MUTEX};
use crate::switch_to_user;

pub fn context_switch() -> ! {
    let frame = schedule();
    unsafe {
        switch_to_user(frame);
    }
}

pub fn schedule() -> usize {
    unsafe {
        let time = crate::cpu::get_mtime();
        if PROCESS_LIST_MUTEX.try_lock() == false {
            println!("can't get lock");
            return 0;
        }
        if let Some(mut pl) = PROCESS_LIST.take() {
            let mut frame_addr: usize = 0;
            let mut pid: usize = 0;
            'procfindloop: loop {
                pl.rotate_left(1);
                if let Some(prc) = pl.front_mut() {
                    match prc.state {
                        ProcessState::Running => {
                            frame_addr = prc.frame as usize;
                            pid = prc.pid as usize;
                            break 'procfindloop;
                        }
                        ProcessState::Sleeping => {
                            if prc.sleep_until.as_u64() > time.as_u64() {
                                // println!(
                                //     "Skipping process {}, it's sleeping until {}",
                                //     prc.pid,
                                //     prc.sleep_until.formatted()
                                // );
                            } else {
                                // println!("Awaking process {}, it's done sleeping", prc.pid);
                                prc.state = ProcessState::Running;
                                frame_addr = prc.frame as usize;
                                pid = prc.pid as usize;
                                break 'procfindloop;
                            }
                        }
                        _ => {
                            // println!("Skipping process {}, it's {:?}", prc.pid, prc.state);
                        }
                    }
                }
            }
            PROCESS_LIST.replace(pl);
            PROCESS_LIST_MUTEX.unlock();

            // if pid > 1 {
            //     println!("### Scheduling {} at {}", pid, time.formatted());
            // }
            return frame_addr;
        } else {
            println!("no proc");
        }
    }

    0
}
