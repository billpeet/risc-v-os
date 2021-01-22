extern crate alloc;
use crate::process;
use crate::syscall;
use alloc::string::String;

pub struct Shell {
    running: bool,
}

impl Shell {
    pub fn new() -> Self {
        Shell { running: true }
    }

    pub fn shell(&mut self) {
        print!("> ");
        let mut cmd = String::from("");
        while self.running {
            let mut buffer = [0 as u8; 255];
            let buffer_ptr = buffer.as_mut_ptr();
            let chars = syscall::sys_read(0, buffer_ptr, 255);
            if chars > 0 {
                for i in 0..chars {
                    match buffer[i] {
                        0 => {
                            // Break on null character
                            break;
                        }
                        8 => {
                            // Backspace
                            if !cmd.is_empty() {
                                cmd.pop();
                            }
                        }
                        10 | 13 => {
                            // Newline or carriage return
                            // execute some command here
                            if !cmd.is_empty() {
                                self.execute_command(&cmd);
                                cmd = String::from("");
                                print!("> ");
                            }
                            break;
                        }
                        _ => {
                            cmd.push(buffer[i] as char);
                        }
                    }
                }
            }
        }
    }

    fn execute_command(&mut self, cmd: &str) {
        match cmd {
            "peanut" => {
                println!("hoho");
            }
            "pagefault" => {
                println!("triggering page fault:");
                unsafe {
                    let v = 0xdeadbeef as *mut u64;
                    v.write_volatile(0);
                }
            }
            "ps" => {
                // task manager
                unsafe {
                    process::PROCESS_LIST_MUTEX.sleep_lock();
                    if let Some(pl) = process::PROCESS_LIST.take() {
                        println!("Task Manager");
                        for p in pl.iter() {
                            println!("pid {}, state {:?}", p.pid, p.state);
                        }
                        process::PROCESS_LIST.replace(pl);
                    }
                    process::PROCESS_LIST_MUTEX.unlock();
                }
            }
            "quit" => {
                println!("quitting shell...");
                self.running = false;
            }
            _ => {
                println!("Unrecognized command '{}'", cmd);
            }
        }
    }
}
