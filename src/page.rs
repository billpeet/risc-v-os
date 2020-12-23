use core::{mem::size_of, ptr::null_mut};
use crate::{HEAP_SIZE, HEAP_START};

static mut ALLOC_START: usize = 0;
const PAGE_ORDER: usize = 12;
pub const PAGE_SIZE: usize = 1 << 12;

/// Align (set to a multiple of some power of 2)
pub const fn align_val(val: usize, order: usize) -> usize {
    let o = (1usize << order) - 1;
    (val + o) & !o
}

#[repr(u8)]
pub enum PageBits {
    Empty = 0,
    Taken = 1 << 0,
    Last = 1 << 1,
}

impl PageBits {
    pub fn val(self) -> u8 {
        self as u8
    }
}

pub struct Page {
    flags: u8,
}

impl Page {

    pub fn is_last(&self) -> bool {
        if self.flags & PageBits::Last.val() != 0 {
            true
        }
        else {
            false
        }
    }

    pub fn is_taken(&self) -> bool {
        if self.flags & PageBits::Taken.val() != 0 {
            true
        }
        else {
            false
        }
    }

    pub fn is_free(&self) -> bool {
        !self.is_taken()
    }

    pub fn clear(&mut self) {
        self.flags = PageBits::Empty.val();
    }

    pub fn set_flag(&mut self, flag: PageBits) {
        self.flags |= flag.val();
    }

    pub fn clear_flag(&mut self, flag: PageBits) {
        self.flags &= !(flag.val());
    }

}

pub fn init() {
    unsafe {
        let num_pages = HEAP_SIZE / PAGE_SIZE;
        let ptr = HEAP_START as *mut Page;
        // Clear all pages
        for i in 0..num_pages {
            (*ptr.add(i)).clear();
        }
        ALLOC_START = align_val(HEAP_START + num_pages * size_of::<Page>(), PAGE_ORDER);
    }
}

pub fn alloc(pages: usize) -> *mut u8 {
    assert!(pages > 0);
    unsafe {
        let num_pages = HEAP_SIZE / PAGE_SIZE;
        let ptr = HEAP_START as *mut Page;
        for i in 0..num_pages - pages {
            let mut found = false;
            if(*ptr.add(i)).is_free() {
                found = true;
                // Found 1 free page - now check if we've got enough contiguous memory
                for j in i..i + pages {
                    if (*ptr.add(j)).is_taken() {
                        found = false;
                        break;
                    }
                }
            }

            if found {
                // Set all pages as taken
                for k in i..i + pages - 1 {
                    (*ptr.add(k)).set_flag(PageBits::Taken);
                }

                // Set last page as Last
                (*ptr.add(i + pages - 1)).set_flag(PageBits::Taken);
                (*ptr.add(i + pages - 1)).set_flag(PageBits::Last);
                return (ALLOC_START + PAGE_SIZE * i) as *mut u8;
            }
        }
    }

    // If we got here, no contiguous allocation is found - return null
    null_mut()
}

pub fn zalloc(pages: usize) -> *mut u8 {
    let ret = alloc(pages);
    if !ret.is_null() {
        let size = (PAGE_SIZE * pages) / 8;
        let big_ptr = ret as *mut u64;
        for i in 0..size {
            unsafe {
                (*big_ptr.add(i)) = 0;
            }
        }
    }

    ret
}

pub fn dealloc(ptr: *mut u8) {
    assert!(!ptr.is_null());
    unsafe {
        let addr = HEAP_START + (ptr as usize - ALLOC_START) / PAGE_SIZE;
        assert!(addr >= HEAP_START && addr < HEAP_START + HEAP_SIZE);
        let mut p = addr as *mut Page;
        while (*p).is_taken() && !(*p).is_last() {
            (*p).clear();
            p = p.add(1);
        }

        assert!((*p).is_last() == true, "Possible double-free detected!");

        (*p).clear();
    }
}

/// Debugging functions
pub fn print_page_allocations() {
    unsafe {
        let num_pages = HEAP_SIZE / PAGE_SIZE;
        let /*mut*/ beg = HEAP_START as *const Page;
        let end = beg.add(num_pages);
        let alloc_beg = ALLOC_START;
        let alloc_end = ALLOC_START + num_pages * PAGE_SIZE;
        println!(
            "META: {:p} -> {:p}\n\
            PHYS: 0x{:x} -> 0x{:x}",
            beg, end, alloc_beg, alloc_end
        );
    }
}

pub fn print_allocated_pages() {
    unsafe {
        let mut ptr = HEAP_START as *mut Page;
        let num_pages = HEAP_SIZE / PAGE_SIZE;
        let end = ptr.add(num_pages);
        let mut found = false;
        while ptr < end {
            if (*ptr).is_taken() {
                found = true;
                println!("Allocated page: {:p}", ptr);
            }
            ptr = ptr.add(1);
        }
        
        if !found {
            println!("(No pages allocated)");
        }
    }
}

pub fn paging_tests() {
    println!("############");
    println!("Paging Tests");
    print_page_allocations();
    print_allocated_pages();

    println!("Allocating 10 pages");
    let p1 = alloc(10);
    println!("Allocated page address: {:p}", p1);
    print_allocated_pages();

    println!("Allocating 5 pages");
    let p2 = alloc(5);
    println!("Allocated page address: {:p}", p2);
    print_allocated_pages();

    println!("Deallocating");
    dealloc(p1);
    print_allocated_pages();
}