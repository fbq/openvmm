// Copyright (C) Microsoft Corporation. All rights reserved.

//! ACPI types.

#![cfg_attr(not(any(test, feature = "std")), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod aspt;
pub mod fadt;
pub mod madt;
pub mod pptt;
pub mod srat;

#[allow(non_camel_case_types)]
mod packed_nums {
    pub type u16_ne = zerocopy::U16<zerocopy::NativeEndian>;
    pub type u32_ne = zerocopy::U32<zerocopy::NativeEndian>;
    pub type u64_ne = zerocopy::U64<zerocopy::NativeEndian>;
}

use self::packed_nums::*;
use core::mem::size_of;
use static_assertions::const_assert_eq;
use zerocopy::AsBytes;
use zerocopy::FromBytes;
use zerocopy::FromZeroes;
use zerocopy::Unaligned;

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, AsBytes, FromBytes, FromZeroes, Unaligned)]
pub struct Rsdp {
    pub signature: [u8; 8], // "RSD PTR "
    pub checksum: u8,       // first 20 bytes
    pub oem_id: [u8; 6],
    pub revision: u8, // 2
    pub rsdt: u32,
    pub length: u32,
    pub xsdt: u64,
    pub xchecksum: u8, // full checksum
    pub rsvd: [u8; 3],
}

const_assert_eq!(size_of::<Rsdp>(), 36);

#[repr(C)]
#[derive(Copy, Clone, Debug, AsBytes, FromBytes, FromZeroes, Unaligned)]
pub struct Header {
    pub signature: [u8; 4],
    pub length: u32_ne,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_tableid: [u8; 8],
    pub oem_revision: u32_ne,
    pub creator_id: u32_ne,
    pub creator_revision: u32_ne,
}

const_assert_eq!(size_of::<Header>(), 36);

/// Marker trait for ACPI Table structs that encodes the table's signature
pub trait Table: AsBytes + Unaligned {
    const SIGNATURE: [u8; 4];
}
