#![no_std]
#![feature(abi_riscv_interrupt)]

use core::{arch::global_asm, mem};

use andes_riscv::{
    register::mmisc_ctl,
    riscv::register::{mcounteren, mie, mstatus, mtvec, stvec::TrapMode},
};

pub use hpm_riscv_rt_macros::{entry, fast, interrupt, pre_init};

pub mod trap;

pub mod header;

/// Parse cfg attributes inside a global_asm call.
macro_rules! cfg_global_asm {
    {@inner, [$($x:tt)*], } => {
        global_asm!{$($x)*}
    };
    (@inner, [$($x:tt)*], #[cfg($meta:meta)] $asm:literal, $($rest:tt)*) => {
        #[cfg($meta)]
        cfg_global_asm!{@inner, [$($x)* $asm,], $($rest)*}
        #[cfg(not($meta))]
        cfg_global_asm!{@inner, [$($x)*], $($rest)*}
    };
    {@inner, [$($x:tt)*], $asm:literal, $($rest:tt)*} => {
        cfg_global_asm!{@inner, [$($x)* $asm,], $($rest)*}
    };
    {$($asms:tt)*} => {
        cfg_global_asm!{@inner, [], $($asms)*}
    };
}

//    ".attribute arch, \"rv64im\"",
cfg_global_asm!(
    // no "c" here, the same as riscv-rt
    ".attribute arch, \"rv32im\"",
    ".section .start, \"ax\"
     .global _start
_start:
     .option push
     .option norelax
     la gp, __global_pointer$
     .option pop
    ",
    "la t1, __stack_safe
     addi sp, t1, -16
     call __pre_init
    ",
    // set sp
    "la t1, __stack_start__
     addi sp, t1, -16",
    "call _start_rust",
    "
1:
    j 1b
    ",
);

// weak functions
cfg_global_asm!(
    ".weak __pre_init
__pre_init:
     ret",
    #[cfg(not(feature = "single-hart"))]
    ".weak _mp_hook
_mp_hook:
    beqz a0, 2f // if hartid is 0, return true
1:  wfi // Otherwise, wait for interrupt in a loop
    j 1b
2:  li a0, 1
    ret",
);

#[no_mangle]
unsafe extern "C" fn _setup_interrupts() {
    use andes_riscv::plic::{Plic, PlicExt};

    extern "C" {
        // Symbol defined in hpm-metapac.
        // The symbol must be in FLASH(XPI) or ILM section.
        static __VECTORED_INTERRUPTS: [u32; 1];
    }

    const PLIC: Plic = unsafe { Plic::from_ptr(0xE4000000 as *mut ()) };

    // clean up plic, it will help while debugging
    PLIC.set_threshold(0);
    for i in 0..1024 {
        PLIC.targetconfig(0)
            .claim()
            .modify(|w| w.set_interrupt_id(i));
    }
    // clear any bits left in plic enable register
    for i in 0..4 {
        PLIC.targetint(0).inten(i).write(|w| w.0 = 0);
    }

    // enable mcycle
    mcounteren::set_cy();

    let vector_addr = __VECTORED_INTERRUPTS.as_ptr() as u32;
    // TrapMode is ignored in mtvec, it's set in CSR_MMISC_CTL
    mtvec::write(vector_addr as usize, TrapMode::Direct);

    // Enable vectored external PLIC interrupt
    {
        PLIC.feature().modify(|w| w.set_vectored(true));
        // CSR_MMISC_CTL = 0x7D0
        // asm!("csrsi 0x7D0, 2");
        mmisc_ctl().modify(|w| w.set_vec_plic(true));

        mstatus::set_mie(); // must enable global interrupt
        mstatus::set_sie(); // and supervisor interrupt
        mie::set_mext(); // and PLIC external interrupt
    }
}

unsafe fn memory_copy_range(dst_start: *mut u32, dst_end: *mut u32, src_start: *const u32) {
    let mut dst = dst_start;
    let mut src = src_start;
    while dst < dst_end {
        *dst = *src;
        dst = dst.add(1);
        src = src.add(1);
    }
}

unsafe fn memory_clear_range(start: *mut u32, end: *mut u32) {
    let mut dst = start;
    while dst < end {
        *dst = 0;
        dst = dst.add(1);
    }
}

#[no_mangle]
unsafe extern "C" fn _start_rust() -> ! {
    andes_riscv::l1c::ic_enable();
    andes_riscv::l1c::dc_enable();
    andes_riscv::l1c::dc_invalidate_all();

    extern "C" {
        fn main() -> !;
    }

    extern "C" {
        static mut __vector_ram_start__: u32;
        static mut __vector_ram_end__: u32;
        static __vector_load_addr__: u32;

        static mut __data_start__: u32;
        static mut __data_end__: u32;
        static __data_load_addr__: u32;

        static mut __fast_text_start__: u32;
        static mut __fast_text_end__: u32;
        static __fast_text_load_addr__: u32;

        static mut __fast_data_start__: u32;
        static mut __fast_data_end__: u32;
        static __fast_data_load_addr__: u32;

        static mut __noncacheable_data_start__: u32;
        static mut __noncacheable_data_end__: u32;
        static __noncacheable_data_load_addr__: u32;

        static mut __bss_start__: u32;
        static mut __bss_end__: u32;

        static mut __fast_bss_start__: u32;
        static mut __fast_bss_end__: u32;

        static mut __noncacheable_bss_start__: u32;
        static mut __noncacheable_bss_end__: u32;
    }

    unsafe {
        memory_copy_range(
            &raw mut __vector_ram_start__,
            &raw mut __vector_ram_end__,
            &raw const __vector_load_addr__,
        );

        memory_copy_range(
            &raw mut __data_start__,
            &raw mut __data_end__,
            &raw const __data_load_addr__,
        );

        memory_copy_range(
            &raw mut __fast_text_start__,
            &raw mut __fast_text_end__,
            &raw const __fast_text_load_addr__,
        );

        memory_copy_range(
            &raw mut __fast_data_start__,
            &raw mut __fast_data_end__,
            &raw const __fast_data_load_addr__,
        );

        memory_copy_range(
            &raw mut __noncacheable_data_start__,
            &raw mut __noncacheable_data_end__,
            &raw const __noncacheable_data_load_addr__,
        );

        memory_clear_range(&raw mut __bss_start__, &raw mut __bss_end__);

        memory_clear_range(&raw mut __fast_bss_start__, &raw mut __fast_bss_end__);

        memory_clear_range(
            &raw mut __noncacheable_bss_start__,
            &raw mut __noncacheable_bss_end__,
        );
    }

    _setup_interrupts();

    // enable FPU
    mstatus::set_fs(mstatus::FS::Clean);
    mstatus::set_fs(mstatus::FS::Initial);

    main()
}
