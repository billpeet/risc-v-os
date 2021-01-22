use crate::block;
use crate::console;
use crate::cpu::TrapFrame;
use crate::cpu::{get_mtime, MachineTime, Registers};
use crate::fs;
use crate::mmu::virt_to_phys;
use crate::process::{delete_process, get_by_pid, set_sleeping, set_waiting};

pub const SYSCALL_EXIT: usize = 93;
pub const SYSCALL_EXIT_GROUP: usize = 94;
pub const SYSCALL_YIELD: usize = 1;
pub const SYSCALL_PUTCHAR: usize = 2;
pub const SYSCALL_DUMP_REGISTERS: usize = 8; // TODO
pub const SYSCALL_SLEEP: usize = 10;
pub const SYSCALL_EXECV: usize = 11; // TODO
pub const SYSCALL_WAIT: usize = 3;
pub const SYSCALL_TEST: usize = 99;
pub const SYSCALL_SYS_READ: usize = 63;
pub const SYSCALL_SYS_WRITE: usize = 64;
pub const SYSCALL_GET_PID: usize = 172;
pub const SYSCALL_BLOCK_READ: usize = 180;
pub const SYSCALL_GET_TIME: usize = 1000;
pub const SYSCALL_GET_INODE: usize = 1001;

extern "C" {
    pub fn make_syscall(
        call_num: usize,
        arg0: usize,
        arg1: usize,
        arg2: usize,
        arg3: usize,
        arg4: usize,
        arg5: usize,
    ) -> usize;
}

pub fn do_make_syscall(
    call_num: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> usize {
    unsafe { make_syscall(call_num, arg0, arg1, arg2, arg3, arg4, arg5) }
}

pub fn exit_process() -> usize {
    do_make_syscall(SYSCALL_EXIT, 0, 0, 0, 0, 0, 0)
}

pub fn yield_process() -> usize {
    do_make_syscall(SYSCALL_YIELD, 0, 0, 0, 0, 0, 0)
}

pub fn sleep(period: usize) -> usize {
    do_make_syscall(SYSCALL_SLEEP, period, 0, 0, 0, 0, 0)
}

pub fn wait_process() -> usize {
    do_make_syscall(SYSCALL_WAIT, 0, 0, 0, 0, 0, 0)
}

pub fn putchar(c: char) -> usize {
    do_make_syscall(SYSCALL_PUTCHAR, c as usize, 0, 0, 0, 0, 0)
}

pub fn test_syscall() -> usize {
    do_make_syscall(SYSCALL_TEST, 0, 0, 0, 0, 0, 0)
}

pub fn sys_read(fd: u16, buf: *const u8, size: usize) -> usize {
    do_make_syscall(SYSCALL_SYS_READ, fd as usize, buf as usize, size, 0, 0, 0)
}

pub fn sys_write(fd: u16, buf: *const u8, size: usize) -> usize {
    do_make_syscall(SYSCALL_SYS_WRITE, fd as usize, buf as usize, size, 0, 0, 0)
}

pub fn get_pid() -> u16 {
    do_make_syscall(SYSCALL_GET_PID, 0, 0, 0, 0, 0, 0) as u16
}

pub fn read_block(dev: usize, buf: *const u8, size: u32, offset: u32) {
    do_make_syscall(
        SYSCALL_BLOCK_READ,
        dev,
        buf as usize,
        size as usize,
        offset as usize,
        0,
        0,
    );
}

pub fn get_time() -> MachineTime {
    let ticks = do_make_syscall(SYSCALL_GET_TIME, 0, 0, 0, 0, 0, 0);
    MachineTime::from_ticks(ticks as u64)
}

pub fn get_inode(dev: usize, node: u32, buffer: *mut u8, size: u32, offset: u32) -> u32 {
    do_make_syscall(
        SYSCALL_GET_INODE,
        dev as usize,
        node as usize,
        buffer as usize,
        size as usize,
        offset as usize,
        0,
    ) as u32
}

pub unsafe fn do_syscall(mepc: usize, frame: *mut TrapFrame) -> bool {
    let syscall_number = (*frame).regs[Registers::A7 as usize];
    let pid = (*frame).pid as u16;

    // Advance process' program counter
    (*frame).pc = mepc + 4;

    match syscall_number {
        SYSCALL_EXIT | SYSCALL_EXIT_GROUP => {
            // Exit process
            delete_process(pid);
            return true;
        }
        SYSCALL_YIELD => {
            // Yield - context switch immediately
            // println!("yielding {}", pid);
            return true;
        }
        SYSCALL_SLEEP => {
            // sleep
            let period_ms = (*frame).regs[Registers::A0 as usize] as u64;
            let period = MachineTime::from_ms(period_ms);
            let sleep_until = get_mtime().offset(period);
            // println!(
            //     "{} wants to sleep for {}, until {}",
            //     pid,
            //     period.formatted(),
            //     sleep_until.formatted()
            // );

            set_sleeping(pid, sleep_until);
            return true;
        }
        SYSCALL_WAIT => {
            // Wait - set as waiting and context switch
            println!("{} wants to wait", pid);
            set_waiting(pid);
            return true;
        }
        SYSCALL_PUTCHAR => {
            // putchar
            println!(
                "{}: {}",
                pid,
                (*frame).regs[Registers::A0 as usize] as u8 as char
            );
        }
        SYSCALL_TEST => {
            // Test syscall
            println!("test syscall from {}", pid);
        }
        SYSCALL_SYS_READ => {
            // sys_read
            let mut reschedule = false;
            let fd = (*frame).regs[Registers::A0 as usize] as u16;
            let mut buf = (*frame).regs[Registers::A1 as usize] as *const u8;
            let size = (*frame).regs[Registers::A2 as usize];
            if fd == 0 {
                // stdin
                let mut ret: usize = 0;
                console::IN_LOCK.spin_lock();
                if let Some(mut inb) = console::IN_BUFFER.take() {
                    let num_elements = if inb.len() >= size { size } else { inb.len() };
                    let proc = get_by_pid(pid);
                    if num_elements == 0 {
                        // nothing available, push to queue and get process to wait
                        console::push_queue(pid);
                        set_waiting(pid);
                        reschedule = true;
                    } else {
                        //
                        for i in inb.drain(0..num_elements) {
                            let paddr = if (*frame).satp >> 60 == 0 {
                                // Running in machine mode - address is already physical
                                Some(buf as usize)
                            } else {
                                // Running in user/supervisor mode - need to lookup physical address
                                let table = ((*proc).root_table).as_mut().unwrap();
                                virt_to_phys(table, buf as usize)
                            };
                            if paddr.is_none() {
                                break;
                            }
                            let buf_ptr = paddr.unwrap() as *mut u8;
                            buf_ptr.write_volatile(i);
                            buf = buf.add(1);
                            ret += 1;
                        }
                    }
                    console::IN_BUFFER.replace(inb);
                }
                console::IN_LOCK.unlock();
                (*frame).regs[Registers::A0 as usize] = ret;
                return reschedule;
            }
        }
        SYSCALL_SYS_WRITE => {
            // sys_write
            let fd = (*frame).regs[Registers::A0 as usize] as u16;
            let buf = (*frame).regs[Registers::A1 as usize] as *const u8;
            let size = (*frame).regs[Registers::A2 as usize];
            if fd == 1 || fd == 2 {
                // stdout / stderr
                let proc = get_by_pid(pid);
                let mut iter = 0;
                for i in 0..size {
                    let paddr = if (*frame).satp >> 60 == 0 {
                        // Running in machine mode - address is already physical
                        Some(buf.add(i) as usize)
                    } else {
                        // Running in user/supervisor mode - need to lookup physical address
                        let table = ((*proc).root_table).as_mut().unwrap();
                        virt_to_phys(table, buf.add(i) as usize)
                    };
                    if let Some(bufaddr) = paddr {
                        print!("{}", *(bufaddr as *const u8) as char);
                        iter += 1;
                    }
                }
                (*frame).regs[Registers::A0 as usize] = iter as usize;
            }
        }
        SYSCALL_GET_PID => {
            // get pid
            (*frame).regs[Registers::A0 as usize] = pid as usize;
        }
        SYSCALL_BLOCK_READ => {
            // read block
            set_waiting(pid);
            let dev = (*frame).regs[Registers::A0 as usize];
            let buffer = (*frame).regs[Registers::A1 as usize] as *mut u8;
            let size = (*frame).regs[Registers::A2 as usize] as u32;
            let offset = (*frame).regs[Registers::A3 as usize] as u64;
            block::block_op(dev, buffer, size, offset, false, pid);
            return true;
        }
        SYSCALL_GET_TIME => {
            // get time
            (*frame).regs[Registers::A0 as usize] = get_mtime().as_u64() as usize;
        }
        SYSCALL_GET_INODE => {
            // get inode
            set_waiting(pid);
            let dev = (*frame).regs[Registers::A0 as usize];
            let node = (*frame).regs[Registers::A1 as usize] as u32;
            let buffer = (*frame).regs[Registers::A2 as usize] as *mut u8;
            let size = (*frame).regs[Registers::A3 as usize] as u32;
            let offset = (*frame).regs[Registers::A4 as usize] as u32;
            fs::process_read(pid, dev, node, buffer, size, offset);
            return true;
        }
        _ => {
            println!("Unknown syscall number {} from {}", syscall_number, pid);
        }
    }

    return false;
}
