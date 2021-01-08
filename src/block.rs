use crate::virtio;
use crate::virtio::{MmioOffsets, StatusField, Queue, Descriptor, VIRTIO_RING_SIZE};
use crate::page::{PAGE_SIZE, zalloc};
use crate::kmem::{kmalloc, kfree};
use core::mem::size_of;

#[repr(C)]
pub struct Header {
    blktype: u32,
    reserved: u32,
    sector: u64,
}

#[repr(C)]
pub struct Data {
    data: *mut u8,
}

#[repr(C)]
pub struct Status {
    status: u8,
}

#[repr(C)]
pub struct Request {
    header: Header,
    data: Data,
    status: Status,
    head: u16,
}

pub struct BlockDevice {
	queue:        *mut Queue,
	dev:          *mut u32,
	idx:          u16,
	ack_used_idx: u16,
	read_only:    bool,
}

// Type values
pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;
pub const VIRTIO_BLK_T_FLUSH: u32 = 4;
pub const VIRTIO_BLK_T_DISCARD: u32 = 11;
pub const VIRTIO_BLK_T_WRITE_ZEROES: u32 = 13;

// Feature bits
pub const VIRTIO_BLK_F_SIZE_MAX: u32 = 1;
pub const VIRTIO_BLK_F_SEG_MAX: u32 = 2;
pub const VIRTIO_BLK_F_GEOMETRY: u32 = 4;
pub const VIRTIO_BLK_F_RO: u32 = 5;
pub const VIRTIO_BLK_F_BLK_SIZE: u32 = 6;
pub const VIRTIO_BLK_F_FLUSH: u32 = 9;
pub const VIRTIO_BLK_F_TOPOLOGY: u32 = 10;
pub const VIRTIO_BLK_F_CONFIG_WCE: u32 = 11;
pub const VIRTIO_BLK_F_DISCARD: u32 = 13;
pub const VIRTIO_BLK_F_WRITE_ZEROES: u32 = 14;

static mut BLOCK_DEVICES: [Option<BlockDevice>; 8] = [None, None, None, None, None, None, None, None];

pub fn setup_block_device(ptr: *mut u32) -> bool {
    unsafe {
        let index = (ptr as usize - virtio::MMIO_VIRTIO_START) >> 12;
        // 1. Reset by writing 0 to status
        ptr.add(MmioOffsets::Status.scale32()).write_volatile(0);
        // 2. Set ACKNOWLEDGE status bit
        let mut status_bits = StatusField::Acknowledge.val32();
        ptr.add(MmioOffsets::Status.scale32()).write_volatile(status_bits);
        // 3. Set the DRIVER status bit
        status_bits |= StatusField::DriverOk.val32();
        ptr.add(MmioOffsets::Status.scale32()).write_volatile(status_bits);
        // 4. Read device feature bits, write subset of feature bits understood by OS and driver to the device
        let host_features = ptr.add(MmioOffsets::HostFeatures.scale32()).read_volatile();
        let guest_features = host_features & !(1 << VIRTIO_BLK_F_RO);
        let ro = host_features & (1 << VIRTIO_BLK_F_RO) != 0;
        ptr.add(MmioOffsets::GuestFeatures.scale32()).write_volatile(guest_features);
        // 5. Set the FEATURES_OK status bit
        status_bits |= StatusField::FeaturesOk.val32();
        ptr.add(MmioOffsets::Status.scale32()).write_volatile(status_bits);
        // 6. Re-read status to ensure FEATURES_OK is still set.
        // Otherwise, it doesn't support our features.
        let status_ok = ptr.add(MmioOffsets::Status.scale32()).read_volatile();
        if false == StatusField::features_ok(status_ok) {
            print!("features fail...");
            ptr.add(MmioOffsets::Status.scale32()).write_volatile(StatusField::Failed.val32());
            return false;
        }

        // 7. Perform device-specific setup
        let qnmax = ptr.add(MmioOffsets::QueueNumMax.scale32()).read_volatile();
        // Max sure the queue size is valid
        if VIRTIO_RING_SIZE as u32 > qnmax {
            print!("queue size fail...");
            return false;
        }
        // Set queue num
        ptr.add(MmioOffsets::QueueNum.scale32()).write_volatile(VIRTIO_RING_SIZE as u32);
        let num_pages = (size_of::<Queue>() + PAGE_SIZE - 1) / PAGE_SIZE;
        ptr.add(MmioOffsets::QueueSel.scale32()).write_volatile(0);
        let queue_ptr = zalloc(num_pages) as *mut Queue;
        let queue_pfn = queue_ptr as u32;
        ptr.add(MmioOffsets::GuestPageSize.scale32()).write_volatile(PAGE_SIZE as u32);
        ptr.add(MmioOffsets::QueuePfn.scale32()).write_volatile(queue_pfn / PAGE_SIZE as u32);
        let bd = BlockDevice {
            queue: queue_ptr,
            dev: ptr,
            idx: 0,
            ack_used_idx: 0,
            read_only: ro,
        };
        BLOCK_DEVICES[index] = Some(bd);
        
        // 8. Set the DRIVER_OK status bit. Device is now 'live'
        status_bits |= StatusField::DriverOk.val32();
        ptr.add(MmioOffsets::Status.scale32()).write_volatile(status_bits);

        true
    }
}

pub fn fill_next_descriptor(bd: &mut BlockDevice, desc: Descriptor) -> u16 {
    unsafe {
        bd.idx = (bd.idx + 1) % VIRTIO_RING_SIZE as u16;
        (*bd.queue).desc[bd.idx as usize] = desc;
        if (*bd.queue).desc[bd.idx as usize].flags & virtio::VIRTIO_DESC_F_NEXT != 0 {
            (*bd.queue).desc[bd.idx as usize].next = (bd.idx + 1) % VIRTIO_RING_SIZE as u16;
        }
        bd.idx
    }
}

pub fn read(dev: usize, buffer: *mut u8, size: u32, offset: u64) {
    block_op(dev, buffer, size, offset, false);
}

pub fn write(dev: usize, buffer: *mut u8, size: u32, offset: u64) {
    block_op(dev, buffer, size, offset, true);
}

pub fn block_op(dev: usize, buffer: *mut u8, size: u32, offset: u64, write: bool) {
    unsafe {
        if let Some(bdev) = BLOCK_DEVICES[dev - 1].as_mut() {
            if true == bdev.read_only && true == write {
                println!("Trying to write to read-only device!");
                return;
            }
            
            let sector = offset / 512;
            // allocate request on the heap
            let blk_request = kmalloc(size_of::<Request>()) as *mut Request;
            let desc = Descriptor {
                addr: &(*blk_request).header as *const Header as u64,
                len: size_of::<Header>() as u32,
                flags: virtio::VIRTIO_DESC_F_NEXT,
                next: 0,
            };
            let head_idx = fill_next_descriptor(bdev, desc);
            (*blk_request).header.sector = sector;
            (*blk_request).header.blktype = if true == write {
                VIRTIO_BLK_T_OUT
            } else {
                VIRTIO_BLK_T_IN
            };
            (*blk_request).header.reserved = 0;
            (*blk_request).data.data = buffer;
            (*blk_request).status.status = 111; // arbitrary status, we'll read it back to see if the device has changed it
            let desc = Descriptor {
                addr: buffer as u64,
                len: size,
                flags: virtio::VIRTIO_DESC_F_NEXT | if false == write {
                    virtio::VIRTIO_DESC_F_WRITE
                } else {
                    0
                },
                next: 0,
            };
            let _data_idx = fill_next_descriptor(bdev, desc);
            let desc = Descriptor {
                addr: &(*blk_request).status as *const Status as u64,
                len: size_of::<Status>() as u32,
                flags: virtio::VIRTIO_DESC_F_WRITE,
                next: 0,
            };
            let _status_idx = fill_next_descriptor(bdev, desc);
            (*bdev.queue).avail.ring[(*bdev.queue).avail.idx as usize % virtio::VIRTIO_RING_SIZE] = head_idx;
            (*bdev.queue).avail.idx = (*bdev.queue).avail.idx.wrapping_add(1);
            bdev.dev.add(MmioOffsets::QueueNotify.scale32()).write_volatile(0);
        }
    }
}

pub fn pending(bd: &mut BlockDevice) {
    unsafe {
        // check the used ring and free descriptor memory
        let ref queue = *bd.queue;
        while bd.ack_used_idx != queue.used.idx {
            let ref elem = queue.used.ring[bd.ack_used_idx as usize];
            bd.ack_used_idx = (bd.ack_used_idx + 1) % VIRTIO_RING_SIZE as u16;
            let rq = queue.desc[elem.id as usize].addr as *const Request;
            kfree(rq as *mut u8);
            // TODO: awake process waiting for I/O
        }
    }
}

pub fn handle_interrupt(dev_id: usize) {
    unsafe {
        println!("handling interrupt");
        if let Some(bdev) = BLOCK_DEVICES[dev_id].as_mut() {
            pending(bdev);
        } else {
            println!("Invalid block device for interrupt {}", dev_id + 1);
        }
    }
}