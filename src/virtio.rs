use crate::{block, block::{setup_block_device}};
use crate::page::{PAGE_SIZE};
use crate::random;
use core::mem::size_of;

// Flags
pub const VIRTIO_DESC_F_NEXT: u16 = 1;
pub const VIRTIO_DESC_F_WRITE: u16 = 2;
pub const VIRTIO_DESC_F_INDIRECT: u16 = 4;

pub const VIRTIO_AVAIL_F_NO_INTERRUPT: u16 = 1;

pub const VIRTIO_USED_F_NO_NOTIFY: u16 = 1;

pub const VIRTIO_RING_SIZE: usize = 1 << 7;

#[repr(C)]
pub struct Descriptor {
	pub addr:  u64,
	pub len:   u32,
	pub flags: u16,
	pub next:  u16,
}

#[repr(C)]
pub struct Available {
	pub flags: u16,
	pub idx:   u16,
	pub ring:  [u16; VIRTIO_RING_SIZE],
	pub event: u16,
}

#[repr(C)]
pub struct UsedElem {
	pub id:  u32,
	pub len: u32,
}

#[repr(C)]
pub struct Used {
	pub flags: u16,
	pub idx:   u16,
	pub ring:  [UsedElem; VIRTIO_RING_SIZE],
	pub event: u16,
}

#[repr(C)]
pub struct Queue {
	pub desc:  [Descriptor; VIRTIO_RING_SIZE],
	pub avail: Available,
	// Calculating padding, we need the used ring to start on a page boundary. We take the page size, subtract the
	// amount the descriptor ring takes then subtract the available structure and ring.
	pub padding0: [u8; PAGE_SIZE - size_of::<Descriptor>() * VIRTIO_RING_SIZE - size_of::<Available>()],
	pub used:     Used,
}

#[repr(usize)]
pub enum MmioOffsets {
    MagicValue = 0x000,
    Version = 0x004,
    DeviceId = 0x008,
    VendorId = 0x00c,
    HostFeatures = 0x010,
    HostFeaturesSel = 0x014,
    GuestFeatures = 0x020,
    GuestFeaturesSel = 0x024,
    GuestPageSize = 0x028,
    QueueSel = 0x030,
    QueueNumMax = 0x034,
    QueueNum = 0x038,
    QueueAlign = 0x03c,
    QueuePfn = 0x040,
    QueueNotify = 0x050,
    InterruptStatus = 0x060,
    InterruptAck = 0x064,
    Status = 0x070,
    Config = 0x100,
}

#[repr(usize)]
pub enum DeviceTypes {
	None = 0,
	Network = 1,
	Block = 2,
	Console = 3,
	Entropy = 4,
	Gpu = 16,
	Input = 18,
	Memory = 24,
}

impl MmioOffsets {
    pub fn val(self) -> usize {
        self as usize
    }

    pub fn scaled(self, scale: usize) -> usize {
        self.val() / scale
    }

    pub fn scale32(self) -> usize {
        self.scaled(4)
    }
}


pub enum StatusField {
	Acknowledge = 1,
	Driver = 2,
	Failed = 128,
	FeaturesOk = 8,
	DriverOk = 4,
	DeviceNeedsReset = 64,
}

impl StatusField {
	pub fn val(self) -> usize {
		self as usize
	}

	pub fn val32(self) -> u32 {
		self as u32
    }

    pub fn test(sf: u32, bit: StatusField) -> bool {
        sf & bit.val32() != 0
    }

    pub fn is_failed(sf: u32) -> bool {
		StatusField::test(sf, StatusField::Failed)
	}

	pub fn needs_reset(sf: u32) -> bool {
		StatusField::test(sf, StatusField::DeviceNeedsReset)
	}

	pub fn driver_ok(sf: u32) -> bool {
		StatusField::test(sf, StatusField::DriverOk)
	}

    pub fn features_ok(sf: u32) -> bool {
        StatusField::test(sf, StatusField::FeaturesOk)
    }
}

pub const MMIO_VIRTIO_START: usize = 0x1000_1000;
pub const MMIO_VIRTIO_END: usize = 0x1000_8000;
pub const MMIO_VIRTIO_STRIDE: usize = 0x1000;
pub const MMIO_VIRTIO_MAGIC: u32 = 0x74_72_69_76; // 'triv' (i.e. 'virt' in little-endian)

pub struct VirtioDevice {
    pub devtype: DeviceTypes,
}

impl VirtioDevice {
    pub const fn new() -> Self {
        VirtioDevice { devtype: DeviceTypes::None }
    }

    pub const fn new_with(devtype: DeviceTypes) -> Self {
        VirtioDevice { devtype }
    }
}

static mut VIRTIO_DEVICES: [Option<VirtioDevice>; 8] = [None, None, None, None, None, None, None, None];

pub fn probe() {
    for addr in (MMIO_VIRTIO_START..=MMIO_VIRTIO_END).step_by(MMIO_VIRTIO_STRIDE) {
        print!("Virtio probing 0x{:08x}...", addr);
        let magicvalue;
        let deviceid;
        let ptr = addr as *mut u32;
        unsafe {
            magicvalue = ptr.read_volatile();
            deviceid = ptr.add(2).read_volatile();
        }

        if MMIO_VIRTIO_MAGIC != magicvalue {
            println!("no virtio.");
        } else if 0 == deviceid {
            println!("not connected.");
        } else {
            match deviceid {
                1 => {
                    // Network device
                    print!("network device...");
                    println!("setup failed.");
                },
                2 => {
                    // Block device
                    print!("block device...");
                    if false == setup_block_device(ptr) {
                        println!("setup failed.");
                    } else {
                        let index = (addr - MMIO_VIRTIO_START) >> 12;
                        unsafe {
                            VIRTIO_DEVICES[index] = Some(VirtioDevice::new_with(DeviceTypes::Block));
                        }
                        println!("setup succeeded!");
                    }
                },
                4 => {
                    print!("random number generator...");
                    if false == random::setup_entropy_device(ptr) {
                        println!("setup failed.");
                    }
                },
                16 => {
                    // GPU device
                    print!("GPU device...");
                    println!("setup failed.");
                },
                18 => {
                    // Input device
                    print!("input device...");
                    println!("setup failed.");
                },
                _ => println!("unknown device type."),
            }
        }

    }
}

pub fn handle_interrupt(interrupt: u32) {
    let index = interrupt as usize - 1;
    unsafe {
        if let Some(vd) = &VIRTIO_DEVICES[index] {
            match vd.devtype {
                DeviceTypes::Block => {
                    block::handle_interrupt(index);
                },
                _ => {
                    println!("Invalid device generated interrupt!");
                },
            };
        }
    }
}