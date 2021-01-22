// Platform Level Interrupt Controller (PLIC)
use crate::{uart, virtio};

const PLIC_PRIORITY: usize = 0x0c00_0000;
const PLIC_PENDING: usize = 0x0c00_1000;
const PLIC_INT_ENABLE: usize = 0x0c00_2000;
const PLIC_THRESHOLD: usize = 0x0c20_0000;
const PLIC_CLAIM: usize = 0x0c20_0004;

// Enable an interrupt
pub fn enable(id: u32) {
    let enables = PLIC_INT_ENABLE as *mut u32;
    let actual_id = 1 << id;
    unsafe {
        enables.write_volatile(enables.read_volatile() | actual_id);
    }
}

// Set priority of an interrupt - a number between 0-7
pub fn set_priority(id: u32, priority: u8) {
    let actual_prior = priority & 0b111; // we only use last 3 bits
    let prior_reg = PLIC_PRIORITY as *mut u32;
    unsafe {
        // Offset is PLIC_PRIORITY + 4 * id (we're using u32)
        prior_reg.add(id as usize).write_volatile(actual_prior as u32);
    }
}

// Set threshold - minimum priority for an interrupt to trigger
pub fn set_threshold(threshold: u8) {
    let actual_thresh = threshold & 0b111; // we only use last 3 bits
    let thresh_reg = PLIC_THRESHOLD as *mut u32;
    unsafe {
        thresh_reg.write_volatile(actual_thresh as u32);
    }
}

// Read next pending interrupt
pub fn next() -> Option<u32> {
    let claim_reg = PLIC_CLAIM as *const u32;
    let claim_no;
    unsafe {
        claim_no = claim_reg.read_volatile();
    }
    if claim_no == 0 {
        None
    } else {
        Some(claim_no)
    }
}

// Mark interrupt as handled
pub fn complete(id: u32) {
    let complete_reg = PLIC_CLAIM as *mut u32;
    unsafe {
        // We just write a u32 to the whole register
        complete_reg.write_volatile(id);
    }
}

// Checks if an interrupt is currently pending
pub fn is_pending(id: u32) -> bool {
    let pend = PLIC_PENDING as *const u32;
    let actual_id = 1 << id;
    let pending_ids;
    unsafe {
        pending_ids = pend.read_volatile();
    }
    actual_id & pending_ids != 0
}

pub fn handle_interrupt() {
    if let Some(interrupt) = next() {
        match interrupt {
            1..=8 => {
                // VIRTIO interrupt
                // println!("virtio interrupt");
                virtio::handle_interrupt(interrupt);
            },
            10 => {
                // UART interrupt
                uart::handle_interrupt();
            },
            _ => {
                println!("Unkown external interrupt: {}", interrupt);
            }
        }
        complete(interrupt);
    }
}