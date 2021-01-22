// Locking routines
use crate::syscall;

pub const DEFAULT_LOCK_SLEEP: usize = 1000;

#[repr(u32)]
pub enum MutexState {
    Unlocked = 0,
    Locked = 1,
}

#[repr(C)]
pub struct Mutex {
    state: MutexState
}

impl<'a> Mutex {
    pub const fn new() -> Self {
        Self { state: MutexState::Unlocked }
    }

    pub fn val(&'a self) -> &'a MutexState {
        &self.state
    }

    // Returns whether mutex is currently locked
    pub fn try_lock(&mut self) -> bool {
        unsafe {
            let state: u32;
            asm!("amoswap.w.aq {}, {}, ({})\n", out(reg) state, in(reg) 1, in(reg) self);
            match state {
                0 => true,
                _ => false,
            }
        }
    }

    // Do NOT use inside interrupt context!
    // Sleeps process until lock is available
    pub fn sleep_lock(&mut self) {
        while !self.try_lock() {
            syscall::sleep(DEFAULT_LOCK_SLEEP);
        }
    }

    // Hangs thread until lock is available
    // Can be safely used in interrupt context
    pub fn spin_lock(&mut self) {
        while !self.try_lock() {}
    }

    // Unlocks mutex
    pub fn unlock(&mut self) {
        unsafe {
            asm!("amoswap.w.rl zero, zero, ({})", in(reg) self);
        }
    }

}