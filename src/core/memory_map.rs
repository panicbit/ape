use core::slice;
use std::ffi::CStr;

use itertools::Itertools;

#[derive(Debug)]
pub struct MemoryMap {
    descriptors: Vec<Descriptor>,
}

impl MemoryMap {
    pub fn empty() -> Self {
        Self {
            descriptors: Vec::new(),
        }
    }

    pub(crate) unsafe fn get_slice(&self, addr: usize, max_len: usize) -> Option<&[u8]> {
        let descriptor = self.find_descriptor(addr)?;

        descriptor.get_slice(addr, max_len)
    }

    pub(crate) unsafe fn get_slice_mut(&self, addr: usize, max_len: usize) -> Option<&mut [u8]> {
        let descriptor = self.find_descriptor(addr)?;

        descriptor.get_slice_mut(addr, max_len)
    }

    fn find_descriptor(&self, addr: usize) -> Option<&Descriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.contains_address(addr))
    }

    pub unsafe fn from_raw(map: *const libretro_sys::MemoryMap) -> Self {
        if map.is_null() {
            return MemoryMap::empty();
        }

        let descriptors =
            slice::from_raw_parts((*map).descriptors, (*map).num_descriptors as usize)
                .iter()
                .map(|descriptor| Descriptor::from_raw_ref(descriptor))
                .collect_vec();

        Self { descriptors }
    }
}

#[derive(custom_debug::Debug)]
pub struct Descriptor {
    flags: u64,
    ptr: *mut u8,
    #[debug(format = "0x{:X}")]
    offset: usize,
    #[debug(format = "0x{:X}")]
    start: usize,
    #[debug(format = "0x{:X}")]
    select: usize,
    #[debug(format = "0x{:X}")]
    disconnect: usize,
    len: usize,
    address_space: String,
}

impl Descriptor {
    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.start + self.len
    }

    pub fn contains_address(&self, addr: usize) -> bool {
        if self.select != 0 {
            // TODO: implement select != 0 case
            return false;
        }

        self.start <= addr && addr < self.end()
    }

    unsafe fn get_raw_slice(&self, addr: usize, max_len: usize) -> Option<(*mut u8, usize)> {
        if addr < self.start || addr >= self.end() {
            return None;
        }

        let offset = addr - self.start;
        let len = (self.len - offset).min(max_len);
        let ptr = self.ptr.byte_add(self.offset + offset);

        Some((ptr, len))
    }

    unsafe fn get_slice(&self, addr: usize, max_len: usize) -> Option<&[u8]> {
        unsafe {
            let (ptr, len) = self.get_raw_slice(addr, max_len)?;
            let slice = slice::from_raw_parts(ptr, len);

            Some(slice)
        }
    }

    unsafe fn get_slice_mut(&self, addr: usize, max_len: usize) -> Option<&mut [u8]> {
        unsafe {
            let (ptr, len) = self.get_raw_slice(addr, max_len)?;
            let slice = slice::from_raw_parts_mut(ptr, len);

            Some(slice)
        }
    }

    unsafe fn from_raw_ref(descriptor: &libretro_sys::MemoryDescriptor) -> Self {
        let address_space = descriptor
            .addrspace
            .as_ref()
            .map(|address_space| CStr::from_ptr(address_space).to_string_lossy().into_owned())
            .unwrap_or_default();

        Self {
            flags: descriptor.flags,
            ptr: descriptor.ptr.cast::<u8>(),
            offset: descriptor.offset,
            start: descriptor.start,
            select: descriptor.select,
            disconnect: descriptor.disconnect,
            len: descriptor.len,
            address_space,
        }
    }
}

fn highest_address_from_mask(mask: usize) -> usize {
    usize::MAX.checked_shr(mask.leading_zeros()).unwrap_or(0)
}
