// BlockBuffer
use crate::{
    cpu::memcpy,
    kmem::{kfree, kmalloc},
};
use core::{
    ops::{Index, IndexMut},
    ptr::null_mut,
};

pub struct Buffer {
    buffer: *mut u8,
    len: usize,
}

impl Buffer {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: kmalloc(size),
            len: size,
        }
    }

    pub fn get_mut(&mut self) -> *mut u8 {
        self.buffer
    }

    pub fn get(&self) -> *const u8 {
        self.buffer
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl Index<usize> for Buffer {
    type Output = u8;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { self.get().add(index).as_ref().unwrap() }
    }
}

impl IndexMut<usize> for Buffer {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { self.get_mut().add(index).as_mut().unwrap() }
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        let mut new = Self {
            buffer: kmalloc(self.len()),
            len: self.len(),
        };
        unsafe {
            memcpy(new.get_mut(), self.get(), self.len());
        }
        new
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if !self.buffer.is_null() {
            kfree(self.buffer);
            self.buffer = null_mut();
        }
    }
}
