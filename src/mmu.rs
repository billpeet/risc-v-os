use crate::{TEXT_START, TEXT_END, RODATA_START, RODATA_END, DATA_START, DATA_END, BSS_START, BSS_END, KERNEL_STACK_START, KERNEL_STACK_END, HEAP_START, HEAP_SIZE};
use crate::page::{zalloc, dealloc, align_val, PAGE_SIZE};
use crate::kmem::{get_page_table, get_head, get_num_allocations};

#[repr(u64)]
#[derive(Copy, Clone)]
pub enum EntryBits {
    None = 0,
    Valid = 1 << 0,
    Read = 1 << 1,
    Write = 1 << 2,
    Execute = 1 << 3,
    User = 1 << 4,
    Global = 1 << 5,
    Access = 1 << 6,
    Dirty = 1 << 7,

    ReadWrite = 1 << 1 | 1 << 2,
    ReadExecute = 1 << 1 | 1 << 3,
    ReadWriteExecute = 1 << 1 | 1 << 2 | 1 << 3,

    UserReadWrite = 1 << 1 | 1 << 2 | 1 << 4,
    UserReadExecute = 1 << 1 | 1 << 3 | 1 << 4,
    UserReadWriteExecute = 1 << 1 | 1 << 2 | 1 << 3 | 1 << 4,
}

impl EntryBits {
    pub fn val(self) -> i64 {
        self as i64
    }
}

pub struct Entry {
    pub entry: i64,
}

impl Entry {
    pub fn is_valid(&self) -> bool {
        self.get_entry() & EntryBits::Valid.val() != 0
    }

    pub fn is_invalid(&self) -> bool {
        !self.is_valid()
    }

    pub fn is_leaf(&self) -> bool {
        self.get_entry() & 0xe != 0
    }

    pub fn is_branch(&self) -> bool {
        !self.is_leaf()
    }

    pub fn set_entry(&mut self, entry: i64) {
        self.entry = entry;
    }

    pub fn get_entry(&self) -> i64 {
        self.entry
    }
}

pub struct Table {
    pub entries: [Entry; 512],
}

impl Table {
    pub fn len() -> usize {
        512
    }
}

// Maps a virtual address to a physical one
pub fn map(root: &mut Table, vaddr: usize, paddr: usize, bits: i64, level: usize) {
    // Read, Write or Execute must be provided
    assert!(bits & 0xe != 0);

    // Extract VPNs from virtual address
    let vpn = [
        // VPN[0] = vaddr[20:12]
        (vaddr >> 12) & 0x1ff,
        // VPN[1] = vaddr[29:21]
        (vaddr >> 21) & 0x1ff,
        // VPN[2] = vaddr[38:30]
        (vaddr >> 30) & 0x1ff,
    ];

    // Extract physical address numbers (PPN) from physical address
    let ppn = [
        // PPN[0] = paddr[20:12]
        (paddr >> 12) & 0x1ff,
        // PPN[1] = paddr[29:21]
        (paddr >> 21) & 0x1ff,
        // PPN[2] = paddr[55:30]
        (paddr >> 30) & 0x3ff_ffff,
    ];

    // Walk the paging tables
    let mut v = &mut root.entries[vpn[2]];
    for i in (level..2).rev() {
        if !v.is_valid() {
            // Not in use - allocate a new physical page
            let page = zalloc(1);
            // Mark as valid
            v.set_entry(
                (page as i64 >> 2)
                | (EntryBits::Valid.val()),
            );
        }

        // Grab paging entry and jump down a level
        let entry = ((v.get_entry() & !0x3ff) << 2) as *mut Entry;
        v = unsafe { entry.add(vpn[i]).as_mut().unwrap() };
    }

    let entry =
        (ppn[2] << 28) as i64 | // PPN[2] = [53:28]
        (ppn[1] << 19) as i64 | // PPN[2] = [27:19]
        (ppn[0] << 10) as i64 | // PPN[2] = [18:10]
        bits |                  // Specified bits
        EntryBits::Valid.val(); // Valid bit
    v.set_entry(entry); // Set the entry
}

// Unmaps a virtual address
pub fn unmap(root: &mut Table) {
    for lv2 in 0..Table::len() {
        let ref entry_lv2 = root.entries[lv2];
        if entry_lv2.is_valid() && entry_lv2.is_branch() {
            // This is a valid entry - get LV1 table
            let memaddr_lv1 = (entry_lv2.get_entry() & !0x3ff) << 2;
            let table_lv1 = unsafe {
                (memaddr_lv1 as *mut Table).as_mut().unwrap()
            };
            for lv1 in 0..Table::len() {
                let ref entry_lv1 = table_lv1.entries[lv1];
                if entry_lv1.is_valid() && entry_lv1.is_branch() {
                    // Get LV0 table and deallocate its page
                    let memaddr_lv0 = (entry_lv1.get_entry() & !0x3ff) << 2;
                    dealloc(memaddr_lv0 as *mut u8);
                }
            }
            // Deallocate LV1 table
            dealloc(memaddr_lv1 as *mut u8);
        }
    }
}

pub fn virt_to_phys(root: &Table, vaddr: usize) -> Option<usize> {
    // Extract VPNs from virtual address
    let vpn = [
        // VPN[0] = vaddr[20:12]
        (vaddr >> 12) & 0x1ff,
        // VPN[1] = vaddr[29:21]
        (vaddr >> 21) & 0x1ff,
        // VPN[2] = vaddr[38:30]
        (vaddr >> 30) & 0x1ff,
    ];

    let mut v = &root.entries[vpn[2]];
    for i in (0..=2).rev() {
        if v.is_invalid() {
            // Invalid entry, page fault
            break;
        }
        else if v.is_leaf() {
            // We've found the leaf entry - get memory address
            let offset_mask = (1 << (12 + i * 9)) - 1;
            let vaddr_pgoffset = vaddr & offset_mask;
            let addr = ((v.get_entry() << 2) as usize) & !offset_mask;
            return Some(addr | vaddr_pgoffset);
        }

        // It's a branch, jump down a level
        let entry = ((v.get_entry() & !0x3ff) << 2) as *const Entry;
        v = unsafe { entry.add(vpn[i - 1]).as_ref().unwrap() };
    }

    // Can't find a leaf address
    None
}

/// Identity maps a physical memory range to virtual
pub fn id_map_range(root: &mut Table, start: usize, end: usize, bits: i64) {
    let mut memaddr = start & !(PAGE_SIZE - 1);
    let num_pages = (align_val(end, 12) - memaddr) / PAGE_SIZE;
    for _ in 0..num_pages {
        map(root, memaddr, memaddr, bits, 0);
        memaddr += 1 << 12;
    }
}

pub fn map_kernel() {
    let root_pt_ptr = get_page_table();
    let root_u = root_pt_ptr as usize;
    let mut root_pt = unsafe { root_pt_ptr.as_mut().unwrap() };
    let kheap_head = get_head() as usize;
    let total_pages = get_num_allocations();
    println!();
    unsafe {
        println!("TEXT:   0x{:x} -> 0x{:x}", TEXT_START, TEXT_END);
        println!("RODATA: 0x{:x} -> 0x{:x}", RODATA_START, RODATA_END);
        println!("DATA:   0x{:x} -> 0x{:x}", DATA_START, DATA_END);
        println!("BSS:    0x{:x} -> 0x{:x}", BSS_START, BSS_END);
        println!("STACK:  0x{:x} -> 0x{:x}", KERNEL_STACK_START, KERNEL_STACK_END);
        println!("HEAP:   0x{:x} -> 0x{:x}", kheap_head, kheap_head + total_pages * 4096);
    }

    // Map kernel heap
    id_map_range(&mut root_pt, kheap_head, kheap_head + total_pages * 4096, EntryBits::ReadWrite.val());

    unsafe {
        // Map heap descriptors
        let num_pages = HEAP_SIZE / PAGE_SIZE;
        id_map_range(&mut root_pt, HEAP_START, HEAP_START + num_pages, EntryBits::ReadWrite.val());

        // Map executable section
        id_map_range(&mut root_pt, TEXT_START, TEXT_END, EntryBits::ReadExecute.val());

        // Map rodata section
        id_map_range(&mut root_pt, RODATA_START, RODATA_END, EntryBits::ReadExecute.val());

        // Map data section
        id_map_range(&mut root_pt, DATA_START, DATA_END, EntryBits::ReadWrite.val());

        // Map bss section
        id_map_range(&mut root_pt, BSS_START, BSS_END, EntryBits::ReadWrite.val());

        // Map kernel stack
        id_map_range(&mut root_pt, KERNEL_STACK_START, KERNEL_STACK_END, EntryBits::ReadWrite.val());
    }

    // Identity map UART
    map(&mut root_pt, 0x1000_0000, 0x1000_0000, EntryBits::ReadWrite.val(), 0);

    // Write root page table to satp register
    let root_ppn = root_u >> 12;
    let satp_val = 8 << 60 | root_ppn;
    unsafe {
        asm!("csrw satp, $0" :: "r"(satp_val));
    }
}