use crate::uart::Uart;
extern crate alloc;
use alloc::string::String;

pub struct Shell {
    uart: Uart,
    cmd: String,
}

impl Shell {

    pub fn new() -> Self {
        Shell {
            uart: Uart::new(0x1000_0000),
            cmd: String::new(),
        }
    }

    pub fn shell(&mut self) {
        print!("> ");
        loop {
            if let Some(c) = self.uart.get() {
                match c {
                    8 => {
                        // Backspace
                        if !self.cmd.is_empty() {
                            print!("{}{}{}", 8 as char, ' ', 8 as char);
                            self.cmd.pop();
                        }
                    },
                    10 | 13 => {
                        // Newline or carriage return
                        // execute some command here
                        println!();
                        if !self.cmd.is_empty() {
                            self.execute_command();
                        }
                        self.cmd = String::new();
                        print!("> ");
                    },
                    0x1b => {
                        if let Some(next_byte) = self.uart.get() {
                            if next_byte == 91 {
                                if let Some(b) = self.uart.get() {
                                    match b as char {
                                        'A' => {
                                            println!("Up arrow");
                                        },
                                        'B' => {
                                            println!("Down arrow");
                                        },
                                        'C' => {
                                            println!("Right arrow");
                                        },
                                        'D' => {
                                            println!("Left arrow");
                                        },
                                        _ => {
                                            println!("Something else");
                                        }
                                    }
                                }
                            }
                        }
                    },
                    _ => {
                        print!("{}", c as char);
                        self.cmd.push(c as char);
                    }
                }
            }
        }
    }

    fn execute_command(&mut self) {
        match self.cmd.as_str() {
            "peanut" => {
                println!("hoho");
            },
            "pagefault" => {
                println!("triggering page fault:");
                unsafe {
                    let v = 0x0 as *mut u64;
                    v.write_volatile(0);
                }
                println!("I'm baaack");
            },
            "timer" => {
                unsafe {
                    // Set the next machine timer to fire.
                    let mtimecmp = 0x0200_4000 as *mut u64;
                    let mtime = 0x0200_bff8 as *const u64;
                    // The frequency given by QEMU is 10_000_000 Hz, so this sets
                    // the next interrupt to fire one second from now.
                    mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000);
                }
            },
            _ => {
                println!("Unrecognized command '{}'", self.cmd.as_str());
            }
        }
    }

}