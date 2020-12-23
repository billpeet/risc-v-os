// kmem.rs
// Sub-page allocation

use crate::page::{align_val, zalloc, PAGE_SIZE};
use crate::mmu::{Table};
use core::{mem::size_of, ptr::null_mut};

#[repr(usize)]
enum AllocListFlags {
    Taken = 1 << 63,
}

impl AllocListFlags {
    pub fn val(self) -> usize {
        self as usize
    }
}

struct AllocList {
    pub flags_size: usize,
}

impl AllocList {
    pub fn is_taken(&self) -> bool {
        self.flags_size & AllocListFlags::Taken.val() != 0
    }

    pub fn is_free(&self) -> bool {
        !self.is_taken()
    }

    pub fn set_taken(&mut self) {
        self.flags_size |= AllocListFlags::Taken.val();
    }

    pub fn set_free(&mut self) {
        self.flags_size &= !AllocListFlags::Taken.val();
    }

    pub fn set_size(&mut self, size: usize) {
        let k = self.is_taken();
        self.flags_size = size & !AllocListFlags::Taken.val();
        if k {
            self.flags_size |= AllocListFlags::Taken.val(); // Restore taken status
        }
    }

    pub fn get_size(&self) -> usize {
        self.flags_size & !AllocListFlags::Taken.val()
    }

}

static mut KMEM_HEAD: *mut AllocList = null_mut();
static mut KMEM_SIZE: usize = 0;
static mut KMEM_PAGE_TABLE: *mut Table = null_mut();

pub fn get_head() -> *mut u8 {
    unsafe { KMEM_HEAD as *mut u8 }
}

pub fn get_page_table() -> *mut Table {
    unsafe { KMEM_PAGE_TABLE as *mut Table }
}

pub fn get_num_allocations() -> usize {
    unsafe { KMEM_SIZE }
}

pub fn init() {
    unsafe {
        // Allocate 64 pages for kernel
        let k_alloc = zalloc(64);
        assert!(!k_alloc.is_null());
        KMEM_SIZE = 64;
        KMEM_HEAD = k_alloc as *mut AllocList;
        (*KMEM_HEAD).set_free();
        (*KMEM_HEAD).set_size(KMEM_SIZE * PAGE_SIZE);

        // Allocate LV2 page table
        KMEM_PAGE_TABLE = zalloc(1) as *mut Table;
    }
}

/// Allocate sub-page level allocation and zero memory
pub fn kzmalloc(size: usize) -> *mut u8 {
    let size = align_val(size, 3);
    let ret = kmalloc(size);

    if !ret.is_null() {
        for i in 0..size {
            unsafe {
                (*ret.add(i)) = 0;
            }
        }
    }
    ret
}

/// Allocate sub-page level allocation
pub fn kmalloc(size: usize) -> *mut u8 {
    unsafe {
        let size = align_val(size, 3) + size_of::<AllocList>();
        let mut head = KMEM_HEAD;
        let tail = (KMEM_HEAD as *mut u8).add(KMEM_SIZE * PAGE_SIZE) as *mut AllocList;

        while head < tail {
            if (*head).is_free() && size <= (*head).get_size() {
                // Here's a spot available
                let chunk_size = (*head).get_size();
                let rem = chunk_size - size;
                (*head).set_taken();
                if rem > size_of::<AllocList>() {
                    // There's some space left over - mark as available
                    let next = (head as *mut u8).add(size) as *mut AllocList;
                    (*next).set_free();
                    (*next).set_size(rem);
                    (*head).set_size(size);
                }
                else {
                    // The space left over isn't big enough, take the entire chunk
                    (*head).set_size(chunk_size);
                }
                return head.add(1) as *mut u8;
            }
            else {
                // Try next chunk
                head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
            }
        }
    }

    null_mut()
}

pub fn kfree(ptr: *mut u8) {
    unsafe {
        if !ptr.is_null() {
            let p = (ptr as *mut AllocList).offset(-1);
            if (*p).is_taken() {
                (*p).set_free();
            }
            coalesce(); // See if we can merge with surrounding chunks to avoid fragmentation
        }
    }
}

pub fn coalesce() {
    unsafe {
        let mut head = KMEM_HEAD;
        let tail = (KMEM_HEAD as *mut u8).add(KMEM_SIZE * PAGE_SIZE) as *mut AllocList;

        while head < tail {
            let next = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
            if (*head).get_size() == 0 {
                // Size is 0 for some reason - jump out to avoid an infinite loop
                break;
            }
            else if next >= tail {
                // The last block's size is incorrect - jump out to avoid page fault
                break;
            }

            if (*head).is_free() && (*next).is_free() {
                // Combine with next block
                (*head).set_size((*head).get_size() + (*next).get_size());
            }

            // Move to next block
            head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
        }
    }
}

// ### Global Allocator
use core::alloc::{GlobalAlloc, Layout};

struct OsGlobalAlloc;

unsafe impl GlobalAlloc for OsGlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        kzmalloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        kfree(ptr);
    }
}

#[global_allocator]
static GA: OsGlobalAlloc = OsGlobalAlloc {};

#[alloc_error_handler]
pub fn alloc_error(l: Layout) -> ! {
    panic!("Allocated failed to allocate {} bytes with {}-byte alignment.", l.size(), l.align());
}

// ##############################
// Kernel memory allocation tests
pub fn print_table() {
    unsafe {
        let mut head = KMEM_HEAD;
        let tail = (KMEM_HEAD as *mut u8).add(KMEM_SIZE * PAGE_SIZE) as *mut AllocList;
        while head < tail {
            println!("{:p}: Length = {:<10} Taken = {}", head, (*head).get_size(), (*head).is_taken());
            head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
        }
    }
}

pub fn kmem_tests() {
    unsafe {
        print_table();
        let i = kmalloc(32) as *mut u32;
        *i = 23;
        println!("Allocated a u32 at {:p} with value {}", i, *i);

        println!("Allocating 5 u64s");
        let i1 = kmalloc(64);
        let i2 = kmalloc(64);
        let i3 = kmalloc(64);
        let i4 = kmalloc(64);
        let i5 = kmalloc(64);
        print_table();

        println!("Deallocating i, i3 and i4");
        kfree(i as *mut u8);
        kfree(i3);
        kfree(i4);
        print_table();

        let i6 = kmalloc(32) as *mut u32;
        println!("Reallocated at {:p}, has value {}", i6, *i6);
        kfree(i6 as *mut u8);

        let i7 = kzmalloc(32) as *mut u32;
        println!("Reallocated at {:p}, has value {}", i7, *i7);

        println!("Deallocating everything");
        kfree(i1);
        kfree(i2);
        kfree(i5);
        kfree(i7 as *mut u8);
        print_table();
    }
}

pub fn global_alloc_tests() {

}