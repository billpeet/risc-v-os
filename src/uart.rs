use core::fmt::{Error, Write};
use core::convert::TryInto;
use crate::console;

pub struct Uart {
    base_address: usize,
}

impl Uart {
    pub fn new(base_address: usize) -> Self {
        Uart {
            base_address
        }
    }
    
    pub fn init(&mut self) {
        let ptr = self.base_address as *mut u8;
        unsafe {
            // Set word length - write 0b11 to LCR (reg 3)
            ptr.add(3).write_volatile((1 << 0) | (1 << 1));

            // Enable FIFO - bit 1 of FCR (reg 2)
            ptr.add(2).write_volatile(1 << 0);

            // Enable receiver buffer interrupts - bit 0 of IER (reg 1)
            ptr.add(1).write_volatile(1 << 0);

            let divisor: u16 = 592;
            let divisor_least: u8 = (divisor & 0xff).try_into().unwrap();
            let divisor_most: u8 = (divisor >> 8).try_into().unwrap();

            // Set divisor latch (7th bit of LCR) to set divisor
            let lcr = ptr.add(3).read_volatile();
            ptr.add(3).write_volatile(lcr | 1 << 7);

            ptr.add(0).write_volatile(divisor_least);
            ptr.add(1).write_volatile(divisor_most);

            // Restore divisor latch
            ptr.add(3).write_volatile(lcr);

        }
    }

    pub fn put(&mut self, c: u8) {
        let ptr = self.base_address as *mut u8;
        unsafe {
            ptr.add(0).write_volatile(c);
        }
    }

    pub fn get(&mut self) -> Option<u8> {
        let ptr = self.base_address as *mut u8;
        unsafe {
            // Read bit 1 of Line Status Register (LSR)
            if ptr.add(5).read_volatile() & 1 == 0 {
                // No data available
                None
            }
            else {
                // Data available
                Some(ptr.add(0).read_volatile())
            }
        }
    }

}

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        for c in s.bytes() {
            self.put(c);
        }
        Ok(())
    }
}

pub fn handle_interrupt() {
    let mut uart = Uart::new(0x1000_0000);
    if let Some(c) = uart.get() {
        console::push_stdin(c);

        match c {
			8 => {
				// This is a backspace, so we
				// essentially have to write a space and
				// backup again:
				print!("{} {}", 8 as char, 8 as char);
			},
			10 | 13 => {
				// Newline or carriage-return
				println!();
			},
			_ => {
				print!("{}", c as char);
			},
		}
    }
}