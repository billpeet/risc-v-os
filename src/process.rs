use crate::cpu::{CpuMode, MachineTime, Registers, TrapFrame};
use crate::lock::Mutex;
use crate::mmu::{map, unmap, EntryBits, Table};
use crate::page::{alloc, dealloc, zalloc, PAGE_SIZE};
use crate::syscall::{exit_process, yield_process};
use alloc::collections::vec_deque::VecDeque;
use core::ptr::null_mut;

// Pages to allocate for stack
const STACK_PAGES: usize = 2;

// Stack address in process' virtual memory
const STACK_ADDR: usize = 0xf_0000_0000;

pub static mut PROCESS_LIST: Option<VecDeque<Process>> = None;
pub static mut PROCESS_LIST_MUTEX: Mutex = Mutex::new();

static mut NEXT_PID: u16 = 1;

// idle process - just constantly yields
fn idle_process() {
    loop {
        yield_process();
        // wfi();
    }
}

pub fn init() {
    unsafe {
        PROCESS_LIST_MUTEX.spin_lock();
        PROCESS_LIST = Some(VecDeque::with_capacity(15));
        PROCESS_LIST_MUTEX.unlock();
        let pid = add_kernel_process(idle_process);
        println!("idle process is {}", pid);
        let pl = PROCESS_LIST.take().unwrap();
        let p = pl.front().unwrap().frame;
        let frame = p as *const TrapFrame as usize;
        println!("Init's frame is at 0x{:08x}", frame);
        PROCESS_LIST.replace(pl);
        PROCESS_LIST_MUTEX.unlock();
    }
}

pub fn set_running(pid: u16) -> bool {
    let mut retval = false;
    unsafe {
        if let Some(mut pl) = PROCESS_LIST.take() {
            for proc in pl.iter_mut() {
                if proc.pid == pid {
                    // println!("awaking {}", pid);
                    proc.state = ProcessState::Running;
                    retval = true;
                    break;
                }
            }
            PROCESS_LIST.replace(pl);
        }
    }
    retval
}

pub fn set_waiting(pid: u16) -> bool {
    let mut retval = false;
    unsafe {
        if let Some(mut pl) = PROCESS_LIST.take() {
            for proc in pl.iter_mut() {
                if proc.pid == pid {
                    // println!("marking {} as waiting", pid);
                    proc.state = ProcessState::Waiting;
                    retval = true;
                    break;
                }
            }
            PROCESS_LIST.replace(pl);
        }
    }
    retval
}

pub fn set_sleeping(pid: u16, sleep_until: MachineTime) -> bool {
    let mut retval = false;
    unsafe {
        if let Some(mut pl) = PROCESS_LIST.take() {
            for proc in pl.iter_mut() {
                if proc.pid == pid {
                    proc.state = ProcessState::Sleeping;
                    proc.sleep_until = sleep_until;
                    retval = true;
                    break;
                }
            }
            PROCESS_LIST.replace(pl);
        }
    }
    retval
}

pub fn delete_process(pid: u16) -> bool {
    let mut retval = false;
    unsafe {
        if let Some(mut pl) = PROCESS_LIST.take() {
            for i in 0..pl.len() {
                let p = pl.get_mut(i).unwrap();
                if (*(*p).frame).pid as u16 == pid {
                    pl.remove(i);
                    retval = true;
                    break;
                }
            }
            PROCESS_LIST.replace(pl);
        }
    }
    retval
}

pub unsafe fn get_by_pid(pid: u16) -> *mut Process {
    let mut ret = null_mut();
    if let Some(mut pl) = PROCESS_LIST.take() {
        for i in pl.iter_mut() {
            if (*(i.frame)).pid as u16 == pid {
                ret = i as *mut Process;
                break;
            }
        }
        PROCESS_LIST.replace(pl);
    }
    ret
}

pub fn add_kernel_process(func: fn()) -> u16 {
    let func_addr = func as usize;
    let func_vaddr = func_addr;
    let my_pid = unsafe { NEXT_PID };
    let mut ret_proc = Process {
        frame: zalloc(1) as *mut TrapFrame,
        stack: alloc(STACK_PAGES),
        pid: my_pid,
        root_table: zalloc(1) as *mut Table,
        state: ProcessState::Running,
        data: ProcessData::zero(),
        sleep_until: MachineTime::zero(),
        program: null_mut(),
        brk: 0,
    };
    unsafe { NEXT_PID += 1 };

    // Move stack pointer to the very bottom of the allocation
    unsafe {
        (*ret_proc.frame).pc = func_vaddr;
        (*ret_proc.frame).regs[Registers::Ra as usize] = ra_delete_proc as usize;
        (*ret_proc.frame).regs[Registers::Sp as usize] =
            ret_proc.stack as usize + PAGE_SIZE * STACK_PAGES;
        (*ret_proc.frame).mode = CpuMode::Machine as usize;
        (*ret_proc.frame).pid = ret_proc.pid as usize;
    }

    if let Some(mut pl) = unsafe { PROCESS_LIST.take() } {
        pl.push_back(ret_proc);
        unsafe {
            PROCESS_LIST.replace(pl);
            PROCESS_LIST_MUTEX.unlock();
        }
        my_pid
    } else {
        // Failed to start process
        unsafe {
            PROCESS_LIST_MUTEX.unlock();
        }
        0
    }
}

pub fn add_kernel_process_args(func: fn(args_ptr: usize), args: usize) -> u16 {
    let func_addr = func as usize;
    let func_vaddr = func_addr;
    let my_pid = unsafe { NEXT_PID };
    let mut ret_proc = Process {
        frame: zalloc(1) as *mut TrapFrame,
        stack: alloc(STACK_PAGES),
        pid: my_pid,
        root_table: zalloc(1) as *mut Table,
        state: ProcessState::Running,
        data: ProcessData::zero(),
        sleep_until: MachineTime::zero(),
        program: null_mut(),
        brk: 0,
    };
    unsafe { NEXT_PID += 1 };

    // Move stack pointer to the very bottom of the allocation
    unsafe {
        (*ret_proc.frame).pc = func_vaddr;
        (*ret_proc.frame).regs[Registers::A0 as usize] = args;
        (*ret_proc.frame).regs[Registers::Ra as usize] = ra_delete_proc as usize;
        (*ret_proc.frame).regs[Registers::Sp as usize] =
            ret_proc.stack as usize + PAGE_SIZE * STACK_PAGES;
        (*ret_proc.frame).mode = CpuMode::Machine as usize;
        (*ret_proc.frame).pid = ret_proc.pid as usize;
    }

    if let Some(mut pl) = unsafe { PROCESS_LIST.take() } {
        pl.push_back(ret_proc);
        unsafe {
            PROCESS_LIST.replace(pl);
            PROCESS_LIST_MUTEX.unlock();
        }
        my_pid
    } else {
        // Failed to start process
        unsafe {
            PROCESS_LIST_MUTEX.unlock();
        }
        0
    }
}

pub fn add_user_process(func: fn()) -> u16 {
    let func_addr = func as usize;
    let func_vaddr = func_addr;
    let my_pid = unsafe { NEXT_PID };
    let mut ret_proc = Process {
        frame: zalloc(1) as *mut TrapFrame,
        stack: alloc(STACK_PAGES),
        pid: my_pid,
        root_table: zalloc(1) as *mut Table,
        state: ProcessState::Running,
        data: ProcessData::zero(),
        sleep_until: MachineTime::zero(),
        program: null_mut(),
        brk: 0,
    };
    unsafe { NEXT_PID += 1 };

    // Move stack pointer to the very bottom of the allocation
    let saddr = ret_proc.stack as usize;
    unsafe {
        (*ret_proc.frame).pc = func_vaddr;
        (*ret_proc.frame).regs[Registers::Ra as usize] = ra_delete_proc as usize;
        (*ret_proc.frame).regs[Registers::Sp as usize] = saddr + PAGE_SIZE * STACK_PAGES;
        (*ret_proc.frame).mode = CpuMode::User as usize;
        (*ret_proc.frame).pid = ret_proc.pid as usize;
    }

    let table_pt;
    unsafe { table_pt = &mut *ret_proc.root_table };

    // Map virtual memory for stack
    for i in 0..STACK_PAGES {
        let addr = i * PAGE_SIZE;
        map(
            table_pt,
            STACK_ADDR + addr,
            saddr + addr,
            EntryBits::UserReadWrite.val(),
            0,
        );
        // println!("Set stack from 0x{:08x} -> 0x{:08x}", STACK_ADDR + addr, saddr + addr);
    }

    // Map program counter
    for i in 0..=100 {
        let modifier = i * 0x1000;
        map(
            table_pt,
            func_vaddr + modifier,
            func_addr + modifier,
            EntryBits::UserReadExecute.val(),
            0,
        );
    }
    // println!("map program counter from 0x{:08x}-0x{:08x} -> 0x{:08x}-0x{:08x}", func_vaddr, func_vaddr + 0x100_000, func_addr, func_addr + 0x100_000);

    // Map syscall function
    map(
        table_pt,
        0x8000_0000,
        0x8000_0000,
        EntryBits::UserReadExecute.val(),
        0,
    );

    // Id map ra_delete_proc function
    map(
        table_pt,
        ra_delete_proc as usize,
        ra_delete_proc as usize,
        EntryBits::UserReadExecute.val(),
        0,
    );

    unsafe {
        PROCESS_LIST_MUTEX.spin_lock();
    }
    if let Some(mut pl) = unsafe { PROCESS_LIST.take() } {
        pl.push_back(ret_proc);
        unsafe {
            PROCESS_LIST.replace(pl);
            PROCESS_LIST_MUTEX.unlock();
        }
        my_pid
    } else {
        // Failed to start process
        unsafe {
            PROCESS_LIST_MUTEX.unlock();
        }
        0
    }
}

fn ra_delete_proc() {
    exit_process();
}

#[derive(Debug)]
pub enum ProcessState {
    Running,
    Sleeping,
    Waiting,
    Dead,
}

#[repr(C)]
pub struct Process {
    pub frame: *mut TrapFrame,
    pub stack: *mut u8,
    pub pid: u16,
    pub root_table: *mut Table,
    pub state: ProcessState,
    pub data: ProcessData,
    pub sleep_until: MachineTime,
    pub program: *mut u8,
    pub brk: usize,
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
