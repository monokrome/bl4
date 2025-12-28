//! Memory address validation helpers
//!
//! Functions for validating UE5 memory pointers.

use crate::memory::source::MemorySource;
use byteorder::{ByteOrder, LE};

/// Check if a pointer looks like a valid UObject (has vtable in code section)
pub fn is_valid_uobject(source: &dyn MemorySource, addr: usize) -> bool {
    if addr < 0x7ff000000000 || addr > 0x7fff00000000 {
        return false; // Not in heap range for this dump
    }
    if let Ok(vtable_data) = source.read_bytes(addr, 8) {
        let vtable = LE::read_u64(&vtable_data) as usize;
        // Vtable should be in code section (0x140... - 0x15f...)
        vtable >= 0x140000000 && vtable < 0x160000000
    } else {
        false
    }
}

/// Check if pointer looks like a property (has FFieldClass in data section)
pub fn is_valid_property(source: &dyn MemorySource, addr: usize) -> bool {
    if addr == 0 {
        return false;
    }
    if let Ok(ffc_data) = source.read_bytes(addr, 8) {
        let ffc = LE::read_u64(&ffc_data) as usize;
        // FFieldClass should be in .didata section (0x14e... - 0x151...)
        ffc >= 0x14e000000 && ffc < 0x152000000
    } else {
        false
    }
}
