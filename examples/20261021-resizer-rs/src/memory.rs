use std::ffi::c_void;

use windows::Win32::System::Memory::{
    MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
    PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_NOACCESS, PAGE_READONLY, PAGE_READWRITE,
    PAGE_WRITECOPY, VirtualQuery,
};

pub unsafe fn read<T: Copy>(address: usize) -> Option<T> {
    if address == 0 || !readable(address, size_of::<T>()) {
        return None;
    }
    Some(unsafe { (address as *const T).read_unaligned() })
}

pub unsafe fn write<T: Copy>(address: usize, value: T) -> bool {
    if address == 0 || !writable(address, size_of::<T>()) {
        return false;
    }
    unsafe {
        (address as *mut T).write_unaligned(value);
    }
    true
}

pub fn readable(address: usize, size: usize) -> bool {
    check_range(address, size, |protection| {
        matches!(
            protection,
            value if value == PAGE_READONLY.0
                || value == PAGE_READWRITE.0
                || value == PAGE_WRITECOPY.0
                || value == PAGE_EXECUTE_READ.0
                || value == PAGE_EXECUTE_READWRITE.0
                || value == PAGE_EXECUTE_WRITECOPY.0
        )
    })
}

pub fn writable(address: usize, size: usize) -> bool {
    check_range(address, size, |protection| {
        protection == PAGE_READWRITE.0
            || protection == PAGE_WRITECOPY.0
            || protection == PAGE_EXECUTE_READWRITE.0
            || protection == PAGE_EXECUTE_WRITECOPY.0
    })
}

pub fn executable(address: usize) -> bool {
    check_range(address, 1, |protection| {
        protection == PAGE_EXECUTE_READ.0
            || protection == PAGE_EXECUTE_READWRITE.0
            || protection == PAGE_EXECUTE_WRITECOPY.0
    })
}

fn check_range(mut address: usize, size: usize, accepts: impl Fn(u32) -> bool) -> bool {
    let Some(end) = address.checked_add(size) else {
        return false;
    };
    if size == 0 {
        return true;
    }

    while address < end {
        let mut info = MEMORY_BASIC_INFORMATION::default();
        let result = unsafe {
            VirtualQuery(
                Some(address as *const c_void),
                &mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if result == 0 || info.State != MEM_COMMIT {
            return false;
        }

        let protection = info.Protect.0;
        if protection & (PAGE_NOACCESS.0 | PAGE_GUARD.0) != 0 || !accepts(protection) {
            return false;
        }

        let next = info.BaseAddress as usize + info.RegionSize;
        if next <= address {
            return false;
        }
        address = next;
    }

    true
}
