// Copyright (C) Microsoft Corporation. All rights reserved.

use crate::tests::common::run_lockable_test;
use crate::tests::common::LockTestBehavior;
use crate::tests::common::RFLAGS_ARITH_MASK;
use iced_x86::code_asm::*;
use x86defs::RFlags;
use x86emu::CpuState;

/// The mask of flags that are changed by an inc/dec operation.
const RFLAGS_INC_MASK: RFlags = RFLAGS_ARITH_MASK.with_carry(false);

fn incdec(variations: &[(u64, u64)], inc: bool) {
    for &(value, rflags) in variations {
        let (state, cpu) = run_lockable_test(
            RFLAGS_INC_MASK,
            LockTestBehavior::Fail,
            |asm| {
                if inc {
                    asm.inc(qword_ptr(rax + 0x10))
                } else {
                    asm.dec(qword_ptr(rax + 0x10))
                }
            },
            |state, cpu| {
                cpu.valid_gva = state.gps[CpuState::RAX].wrapping_add(0x10);
                cpu.mem_val = value;
            },
        );

        assert_eq!(
            cpu.mem_val,
            value.wrapping_add_signed(if inc { 1 } else { -1 })
        );
        assert_eq!(state.rflags & RFLAGS_INC_MASK, rflags.into());
    }
}

#[test]
fn inc() {
    // (value, rflags)
    let variations = &[
        (0, 0),
        (1, 0),
        (2, 0x4),
        (0xf, 0x10),
        (0x10, 0x4),
        (!0, 0x54),
    ];
    incdec(variations, true)
}

#[test]
fn dec() {
    // (value, rflags)
    let variations = &[
        (0, 0x94),
        (1, 0x44),
        (2, 0),
        (0xf, 0),
        (0x10, 0x14),
        (!0, 0x80),
    ];
    incdec(variations, false)
}