use crate::cpu::TrapFrame;
use crate::page::{alloc, zalloc, dealloc, PAGE_SIZE};
use crate::mmu::{Table, EntryBits, map, unmap};
use alloc::collections::vec_deque::VecDeque;

// Pages to allocate for stack
const STACK_PAGES: usize = 2;

// Stack address in process' virtual memory
const STACK_ADDR: usize = 0xf_0000_0000;

pub static mut PROCESS_LIST: Option<VecDeque<Process>> = None;

static mut NEXT_PID: u16 = 1;

extern "C" {
    fn make_syscall(call_num: usize) -> usize;
}

fn init_process() {
    let mut i: usize = 0;
    loop {
        i += 1;
        if i > 70_000_000 {
            unsafe {
                make_syscall(1);
            }
            i = 0;
        }
    }
}

fn process_2() {
    let mut i: usize = 0;
    loop {
        i += 1;
        if i > 70_000_000 {
            unsafe {
                make_syscall(0);
            }
            i = 0;
        }
    }
}

pub fn add_process_default(pr: fn()) {
    unsafe {
        if let Some(mut pl) = PROCESS_LIST.take() {
            let p = Process::new_default(pr);
            pl.push_back(p);
            PROCESS_LIST.replace(pl);
        }
    }
}

pub fn init() -> usize {
    unsafe {
        PROCESS_LIST = Some(VecDeque::with_capacity(15));
        add_process_default(init_process);
        let pl = PROCESS_LIST.take().unwrap();
        let p = pl.front().unwrap().frame;
        let func_vaddr = pl.front().unwrap().program_counter;
        let frame = p as *const TrapFrame as usize;
        println!("Init's frame is at 0x{:08x}", frame);
        PROCESS_LIST.replace(pl);

        add_process_default(process_2);

        func_vaddr
    }
}

pub enum ProcessState {
    Running,
    Sleeping,
    Waiting,
    Dead,
}

#[repr(C)]
pub struct Process {
    frame:          *mut TrapFrame,
    stack:          *mut u8,
    program_counter:usize,
    pid:            u16,
    root_table:     *mut Table,
    state:          ProcessState,
    data:           ProcessData,
    sleep_until:    usize,
}

impl Process {
    pub fn new_default(func: fn()) -> Self {
        let func_addr = func as usize;
        let func_vaddr = func_addr;
        let mut ret_proc = Process {
            frame: zalloc(1) as *mut TrapFrame,
            stack: alloc(STACK_PAGES),
            program_counter: func_vaddr,
            pid: unsafe { NEXT_PID },
            root_table: zalloc(1) as *mut Table,
            state: ProcessState::Running,
            data: ProcessData::zero(),
            sleep_until: 0
        };
        unsafe { NEXT_PID += 1 };

        // Move stack pointer to the very bottom of the allocation
        let saddr = ret_proc.stack as usize;
        unsafe {
            (*ret_proc.frame).regs[2] = STACK_ADDR + PAGE_SIZE * STACK_PAGES;
        }

        let table_pt;
        unsafe { table_pt = &mut *ret_proc.root_table };

        // Map virtual memory for stack
        for i in 0..STACK_PAGES {
            let addr = i * PAGE_SIZE;
            map(table_pt, STACK_ADDR + addr, saddr + addr, EntryBits::UserReadWrite.val(), 0);
            // println!("Set stack from 0x{:08x} -> 0x{:08x}", STACK_ADDR + addr, saddr + addr);
        }

        // Map program counter
        for i in 0..=100 {
            let modifier = i * 0x1000;
            map(table_pt, func_vaddr + modifier, func_addr + modifier, EntryBits::UserReadExecute.val(), 0);
        }
        // println!("map program counter from 0x{:08x}-0x{:08x} -> 0x{:08x}-0x{:08x}", func_vaddr, func_vaddr + 0x100_000, func_addr, func_addr + 0x100_000);

        // Map syscall function
        map(table_pt, 0x8000_0000, 0x8000_0000, EntryBits::UserReadExecute.val(), 0);

        ret_proc
    }

    pub fn get_frame_address(&self) -> usize {
        self.frame as usize
    }

    pub fn get_program_counter(&self) -> usize {
        self.program_counter as usize
    }

    pub fn get_table_address(&self) -> usize {
        self.root_table as usize
    }
    
    pub fn get_state(&self) -> &ProcessState {
        &self.state
    }

    pub fn get_pid(&self) -> u16 {
        self.pid
    }

    // pub fn get_sleep_until(&self) -> usize {
    //     self.sleep_until
    // }

}

impl Drop for Process {
    fn drop(&mut self) {
        // deallocate our stack
        dealloc(self.stack);
        // unmap and deallocate the mmu table
        unsafe {
            unmap(&mut *self.root_table);
        }
        dealloc(self.root_table as *mut u8);
    }
}

// Private data containing metadata about the process, e.g. file name or open file descriptors
pub struct ProcessData {
    cwd_path: [u8; 128],
}

impl ProcessData {
    pub fn zero() -> Self {
        ProcessData { cwd_path: [0; 128] }
    }
}