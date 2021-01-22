// File system
use crate::block::SECTOR_SIZE;
use crate::buffer::Buffer;
use crate::cpu::{memcpy, Registers};
use crate::process;
use crate::syscall::read_block;
use alloc::boxed::Box;
use core::mem::size_of;

pub const MAGIC: u16 = 0x4d5a;
pub const BLOCK_SIZE: u32 = 1024;
pub const NUM_IPTRS: usize = BLOCK_SIZE as usize / 4;

// mode types
pub const S_IFDIR: u16 = 0o040_000;
pub const S_IFREG: u16 = 0o100_000;

#[repr(C)]
pub struct SuperBlock {
    pub ninodes: u32,
    pub pad0: u16,
    pub imap_blocks: u16,
    pub zmap_blocks: u16,
    pub first_data_zone: u16,
    pub log_zone_size: u16,
    pub pad1: u16,
    pub max_size: u32,
    pub zones: u32,
    pub magic: u16,
    pub pad2: u16,
    pub block_size: u16,
    pub version: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Inode {
    pub mode: u16,
    pub hard_links: u16,
    pub uid: u16,
    pub gid: u16,
    pub size: u32,
    pub accessed_time: u32,
    pub modified_time: u32,
    pub creation_time: u32,
    pub zones: [u32; 10],
}

#[repr(C)]
pub struct DirEntry {
    pub inode: u32,
    pub name: [u8; 60],
}

pub struct MinixFileSystem;

impl MinixFileSystem {
    pub fn get_inode(bdev: usize, inode_num: u32) -> Option<Inode> {
        let mut buffer = Buffer::new(BLOCK_SIZE as usize);
        let super_block = unsafe { &*(buffer.get_mut() as *mut SuperBlock) };
        let inode = buffer.get_mut() as *mut Inode;
        read_block(bdev, buffer.get_mut(), SECTOR_SIZE, BLOCK_SIZE); // super block is immediately after boot block
        if super_block.magic == MAGIC {
            // this is a valid disk
            // need to skip boot block, super block and imap/zmap blocks (1024 each)
            let inode_offset = (2 + super_block.imap_blocks + super_block.zmap_blocks) as usize
                * BLOCK_SIZE as usize
                + ((inode_num as usize - 1) / (BLOCK_SIZE as usize / size_of::<Inode>()))
                    * BLOCK_SIZE as usize;

            read_block(bdev, buffer.get_mut(), BLOCK_SIZE, inode_offset as u32);

            let offset = (inode_num as usize - 1) % (BLOCK_SIZE as usize / size_of::<Inode>());
            return unsafe { Some(*inode.add(offset)) }; // Copy across
        }
        None
    }

    pub fn read(bdev: usize, inode: &Inode, buffer: *mut u8, size: u32, offset: u32) -> u32 {
        let mut blocks_seen: u32 = 0;
        let offset_block = offset / BLOCK_SIZE;
        let mut offset_byte = offset % BLOCK_SIZE;
        let mut bytes_left = if size > inode.size { inode.size } else { size };

        let mut bytes_read: u32 = 0;
        // block buffer
        let mut block_buffer = Buffer::new(BLOCK_SIZE as usize);
        // triply indirect zones
        let mut indirect_buffer = Buffer::new(BLOCK_SIZE as usize);
        let mut iindirect_buffer = Buffer::new(BLOCK_SIZE as usize);
        let mut iiindirect_buffer = Buffer::new(BLOCK_SIZE as usize);
        let izones = indirect_buffer.get() as *const u32;
        let iizones = iindirect_buffer.get() as *const u32;
        let iiizones = iiindirect_buffer.get() as *const u32;

        // Read 7 direct zones
        for i in 0..7 {
            if inode.zones[i] == 0 {
                continue;
            }

            if offset_block <= blocks_seen {
                let zone_offset = inode.zones[i] * BLOCK_SIZE;
                read_block(bdev, block_buffer.get_mut(), BLOCK_SIZE, zone_offset);

                let amount_to_read = if BLOCK_SIZE - offset_byte > bytes_left {
                    bytes_left
                } else {
                    BLOCK_SIZE - offset_byte
                };
                unsafe {
                    // copy from buffer to final destination
                    memcpy(
                        buffer.add(bytes_read as usize),
                        block_buffer.get().add(offset_byte as usize),
                        amount_to_read as usize,
                    );
                }
                offset_byte = 0;
                bytes_read += amount_to_read;
                bytes_left -= amount_to_read;
                if bytes_left == 0 {
                    // no more data to read, we're done
                    return bytes_read;
                }
            }
            blocks_seen += 1;
        }

        // Singly indirect zones
        if inode.zones[7] != 0 {
            read_block(
                bdev,
                indirect_buffer.get_mut(),
                BLOCK_SIZE,
                BLOCK_SIZE * inode.zones[7],
            );
            // each indirect zone is list of pointers, 4 bytes each
            for i in 0..NUM_IPTRS {
                unsafe {
                    if izones.add(i).read() != 0 {
                        if offset_block <= blocks_seen {
                            read_block(
                                bdev,
                                block_buffer.get_mut(),
                                BLOCK_SIZE,
                                BLOCK_SIZE * izones.add(i).read(),
                            );
                            let amount_to_read = if BLOCK_SIZE - offset_byte > bytes_left {
                                bytes_left
                            } else {
                                BLOCK_SIZE - offset_byte
                            };
                            // copy from buffer to final destination
                            memcpy(
                                buffer.add(bytes_read as usize),
                                block_buffer.get().add(offset_byte as usize),
                                amount_to_read as usize,
                            );
                            offset_byte = 0;
                            bytes_read += amount_to_read;
                            bytes_left -= amount_to_read;
                            if bytes_left == 0 {
                                return bytes_read;
                            }
                        }
                        blocks_seen += 1;
                    }
                }
            }
        }

        // Doubly indirect zones
        if inode.zones[8] != 0 {
            read_block(
                bdev,
                indirect_buffer.get_mut(),
                BLOCK_SIZE,
                BLOCK_SIZE * inode.zones[8],
            );
            unsafe {
                for i in 0..NUM_IPTRS {
                    if izones.add(i).read() != 0 {
                        read_block(
                            bdev,
                            iindirect_buffer.get_mut(),
                            BLOCK_SIZE,
                            BLOCK_SIZE * izones.add(i).read(),
                        );
                        for j in 0..NUM_IPTRS {
                            if iizones.add(j).read() != 0 {
                                if offset_block <= blocks_seen {
                                    read_block(
                                        bdev,
                                        block_buffer.get_mut(),
                                        BLOCK_SIZE,
                                        BLOCK_SIZE * iizones.add(j).read(),
                                    );
                                    let amount_to_read = if BLOCK_SIZE - offset_byte > bytes_left {
                                        bytes_left
                                    } else {
                                        BLOCK_SIZE - offset_byte
                                    };
                                    // copy from buffer to final destination
                                    memcpy(
                                        buffer.add(bytes_read as usize),
                                        block_buffer.get().add(offset_byte as usize),
                                        amount_to_read as usize,
                                    );
                                    offset_byte = 0;
                                    bytes_read += amount_to_read;
                                    bytes_left -= amount_to_read;
                                    if bytes_left == 0 {
                                        return bytes_read;
                                    }
                                }
                                blocks_seen += 1;
                            }
                        }
                    }
                }
            }
        }

        // Triply indirect zones
        if inode.zones[9] != 0 {
            read_block(
                bdev,
                indirect_buffer.get_mut(),
                BLOCK_SIZE,
                BLOCK_SIZE * inode.zones[9],
            );
            for i in 0..NUM_IPTRS {
                unsafe {
                    if izones.add(i).read() != 0 {
                        read_block(
                            bdev,
                            iindirect_buffer.get_mut(),
                            BLOCK_SIZE,
                            BLOCK_SIZE * izones.add(i).read(),
                        );
                        for j in 0..NUM_IPTRS {
                            if iizones.add(j).read() != 0 {
                                read_block(
                                    bdev,
                                    iiindirect_buffer.get_mut(),
                                    BLOCK_SIZE,
                                    BLOCK_SIZE * iizones.add(i).read(),
                                );
                                for k in 0..NUM_IPTRS {
                                    if iiizones.add(k).read() != 0 {
                                        if offset_block <= blocks_seen {
                                            read_block(
                                                bdev,
                                                block_buffer.get_mut(),
                                                BLOCK_SIZE,
                                                BLOCK_SIZE * iiizones.add(k).read(),
                                            );
                                            let amount_to_read =
                                                if BLOCK_SIZE - offset_byte > bytes_left {
                                                    bytes_left
                                                } else {
                                                    BLOCK_SIZE - offset_byte
                                                };
                                            // copy from buffer to final destination
                                            memcpy(
                                                buffer.add(bytes_read as usize),
                                                block_buffer.get().add(offset_byte as usize),
                                                amount_to_read as usize,
                                            );
                                            offset_byte = 0;
                                            bytes_read += amount_to_read;
                                            bytes_left -= amount_to_read;
                                            if bytes_left == 0 {
                                                return bytes_read;
                                            }
                                        }
                                        blocks_seen += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        bytes_read
    }
}

// reads contents of a specified inode to memory
// don't run inside interrupt context - will block
pub fn read_inode(bdev: usize, node: u32, buffer: *mut u8, size: u32, offset: u32) -> u32 {
    let inode_option = MinixFileSystem::get_inode(bdev, node);
    if let Some(inode) = inode_option {
        return MinixFileSystem::read(bdev, &inode, buffer, size, offset);
    }

    // Invalid node - return 0
    0
}

struct ProcArgs {
    pub pid: u16,
    pub dev: usize,
    pub buffer: *mut u8,
    pub size: u32,
    pub offset: u32,
    pub node: u32,
}

// run inside the read process
fn read_proc(args_addr: usize) {
    let args = unsafe { Box::from_raw(args_addr as *mut ProcArgs) };
    let bytes = read_inode(args.dev, args.node, args.buffer, args.size, args.offset);

    // set return address
    unsafe {
        let ptr = process::get_by_pid(args.pid);
        if !ptr.is_null() {
            (*(*ptr).frame).regs[Registers::A0 as usize] = bytes as usize;
        }
    }

    process::set_running(args.pid);
}

// called by syscall - marks current process as waiting, and spawns a new process to read node
pub fn process_read(pid: u16, dev: usize, node: u32, buffer: *mut u8, size: u32, offset: u32) {
    let args = ProcArgs {
        pid,
        dev,
        node,
        buffer,
        size,
        offset,
    };
    let boxed_args = Box::new(args);
    process::set_waiting(pid);
    process::add_kernel_process_args(read_proc, Box::into_raw(boxed_args) as usize);
}
