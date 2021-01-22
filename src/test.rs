use crate::block::SECTOR_SIZE;
use crate::process::{add_kernel_process, add_user_process};
use crate::syscall::{
    exit_process, get_inode, get_pid, /*get_time, putchar,*/ read_block, sleep, sys_write,
    test_syscall, /*wait_process,*/ yield_process,
};
use crate::{block, kmem, shell};

pub fn init_processes() {
    // add_user_process(process_that_exits);
    // add_user_process(process_2);
    // add_user_process(process_sleepy);
    // add_kernel_process(kernel_block_process);
    // add_kernel_process(process_shell);
    // add_user_process(user_proc);
    add_kernel_process(minix_tester);
}

pub fn process_that_exits() {
    let s = "Welcome to the process that exits!\r\n";
    sys_write(1, s.as_ptr(), s.len());
    println!("exiting now");
}

pub fn minix_tester() {
    let buffer = kmem::kmalloc(1024);
    let root_dir = crate::fs::MinixFileSystem::get_inode(3, 1);
    println!(
        "root dir: {}",
        if root_dir.unwrap().mode & crate::fs::S_IFDIR != 0 {
            "Directory"
        } else {
            "File"
        }
    );
    let bytes_read = get_inode(3, 1, buffer, 1024, 0);
    if bytes_read > 0 {
        println!("bytes read: {}", bytes_read);
        print!("'");
        for i in 0..bytes_read as usize {
            print!("{}", unsafe { buffer.add(i).read() });
        }
        println!("'");

        let dir_entry = buffer as *const crate::fs::DirEntry;
        unsafe {
            for i in 0..bytes_read as usize / 64 {
                let curr = dir_entry.add(i);
                println!("inode: {}", (*curr).inode);
                for j in 0..60 {
                    let c = (*curr).name[j];
                    if c == 0 {
                        break;
                    }
                    print!("{}", c as char);
                }
                println!();
            }
        }
    } else {
        println!("couldn't retrieve inode - invalid disk");
    }

    let bytes_read = get_inode(3, 2, buffer, 100, 0);
    if bytes_read > 0 {
        println!("bytes read: {}", bytes_read);
        print!("'");
        for i in 0..bytes_read as usize {
            print!("{}", unsafe { buffer.add(i).read() as char });
        }
        println!("'");
    } else {
        println!("couldn't retrieve inode - invalid disk");
    }
    kmem::kfree(buffer);
}

pub fn user_proc() {
    let buffer = kmem::kmalloc(1024);
    read_block(3, buffer, SECTOR_SIZE, 1024);
    println!("done from reading blocks");

    unsafe {
        print!("  ");
        for i in 0..16 {
            print!("{:02x} ", buffer.add(i).read());
        }
        println!();
        print!("  ");
        for i in 0..16 {
            print!("{:02x} ", buffer.add(16 + i).read());
        }
        println!();
        print!("  ");
        for i in 0..16 {
            print!("{:02x} ", buffer.add(32 + i).read());
        }
        println!();
        print!("  ");
        for i in 0..16 {
            print!("{:02x} ", buffer.add(48 + i).read());
        }
        println!();
        buffer.add(0).write(0xaa);
        buffer.add(1).write(0xbb);
        buffer.add(2).write(0x7a);
    }
}

pub fn kernel_block_process() {
    let pid = get_pid();
    let buffer = kmem::kmalloc(1024);
    unsafe { buffer.write_volatile(1) }
    let s = "reading block\r\n";
    sys_write(1, s.as_ptr(), s.len());
    block::process_read(pid, 3, buffer, SECTOR_SIZE, 1024);
    let s = "we're back from reading the block\r\n";
    sys_write(1, s.as_ptr(), s.len());

    unsafe {
        print!("  ");
        for i in 0..16 {
            print!("{:02x} ", buffer.add(i).read());
        }
        println!();
        print!("  ");
        for i in 0..16 {
            print!("{:02x} ", buffer.add(16 + i).read());
        }
        println!();
        print!("  ");
        for i in 0..16 {
            print!("{:02x} ", buffer.add(32 + i).read());
        }
        println!();
        print!("  ");
        for i in 0..16 {
            print!("{:02x} ", buffer.add(48 + i).read());
        }
        println!();
        buffer.add(0).write(0xaa);
        buffer.add(1).write(0xbb);
        buffer.add(2).write(0x7a);
    }

    let s = "writing block\r\n";
    sys_write(1, s.as_ptr(), s.len());
    block::process_write(pid, 3, buffer, SECTOR_SIZE, 0);
    let s = "we're back from writing to the block\r\n";
    sys_write(1, s.as_ptr(), s.len());

    kmem::kfree(buffer);
}

pub fn process_2() {
    let mut i: usize = 0;
    loop {
        i += 1;
        if i > 70_000_000 {
            test_syscall();
            let s = "Welcome to Process 2\r\n";
            sys_write(1, s.as_ptr(), s.len());
            i = 0;
        }
    }
}

pub fn process_sleepy() {
    let mut i: usize = 0;
    loop {
        i += 1;
        if i > 70_000_000 {
            sleep(3_000);
            test_syscall();
            i = 0;
        }
    }
}

pub fn process_waiting() {
    // yield once we're done
    loop {
        yield_process();
    }
}

pub fn process_shell() {
    // Shell process
    let mut shell = shell::Shell::new();
    shell.shell();
}
