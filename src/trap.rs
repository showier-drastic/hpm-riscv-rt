use core::arch::global_asm;

use andes_riscv::riscv::register::mcause;

/// Registers saved in trap handler
#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug)]
pub struct TrapFrame {
    pub ra: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
}

#[no_mangle]
#[allow(unused_variables, non_snake_case)]
pub fn DefaultExceptionHandler(trap_frame: &TrapFrame) -> ! {
    loop {
        // Prevent this from turning into a UDF instruction
        // see rust-lang/rust#28728 for details
        continue;
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub fn DefaultInterruptHandler() {
    loop {
        // Prevent this from turning into a UDF instruction
        // see rust-lang/rust#28728 for details
        continue;
    }
    // it's ok to use this both as Core and External interrupt handler, because it never returns
}

extern "C" {
    fn InstructionMisaligned(trap_frame: &TrapFrame);
    fn InstructionFault(trap_frame: &TrapFrame);
    fn IllegalInstruction(trap_frame: &TrapFrame);
    fn Breakpoint(trap_frame: &TrapFrame);
    fn LoadMisaligned(trap_frame: &TrapFrame);
    fn LoadFault(trap_frame: &TrapFrame);
    fn StoreMisaligned(trap_frame: &TrapFrame);
    fn StoreFault(trap_frame: &TrapFrame);
    fn UserEnvCall(trap_frame: &TrapFrame);
    fn SupervisorEnvCall(trap_frame: &TrapFrame);
    fn MachineEnvCall(trap_frame: &TrapFrame);
    fn InstructionPageFault(trap_frame: &TrapFrame);
    fn LoadPageFault(trap_frame: &TrapFrame);
    fn StorePageFault(trap_frame: &TrapFrame);
}

// UM: 4.1.1
#[doc(hidden)]
#[no_mangle]
pub static __EXCEPTIONS: [Option<unsafe extern "C" fn(&TrapFrame)>; 16] = [
    // Instruction Address misaligned
    Some(InstructionMisaligned),
    // Instruction access fault
    Some(InstructionFault),
    Some(IllegalInstruction),
    Some(Breakpoint),
    // Load address misaligned
    Some(LoadMisaligned),
    // Load access fault
    Some(LoadFault),
    // Store/AMO address misaligned
    Some(StoreMisaligned),
    // Store/AMO access fault
    Some(StoreFault),
    // Environment call from U-mode
    Some(UserEnvCall),
    // Environment call from S-mode
    Some(SupervisorEnvCall),
    None,
    Some(MachineEnvCall),
    Some(InstructionPageFault),
    Some(LoadPageFault),
    None,
    // Store/AMO page fault
    Some(StorePageFault),
];

extern "C" {
    fn SupervisorSoft();
    // generated by PLICSW
    fn MachineSoft();
    fn SupervisorTimer();
    // generated by MCHTMR
    fn MachineTimer();
    fn SupervisorExternal();
    fn MachineExternal();
    // fn Coprocessor(); = 12
    // fn Host(); = 13
}

#[doc(hidden)]
#[no_mangle]
pub static __INTERRUPTS: [Option<unsafe extern "C" fn()>; 14] = [
    None,
    Some(SupervisorSoft),
    None, // HypervisorSoft
    Some(MachineSoft),
    None,
    Some(SupervisorTimer),
    None, // HyperVisorTimer
    Some(MachineTimer),
    None,
    Some(SupervisorExternal),
    None, // HypervisorExternal
    Some(MachineExternal),
    None, // Coprocessor
    None, // Host
];

// Interrupts vector index 0, the CORE_LOCAL interrupt
#[no_mangle]
#[allow(non_snake_case)]
#[link_section = ".isr_vector"]
unsafe extern "C" fn _start_rust_CORE_LOCAL(trap_frame: *const TrapFrame) {
    extern "C" {
        fn ExceptionHandler(trap_frame: &TrapFrame);
        fn DefaultHandler();
    }

    let cause = mcause::read();
    let code = cause.code();

    if cause.is_exception() {
        // Ref: HPM6700_6400_Errata_V2_0.pdf "E00001：RISC-V 处理器指令和数据本地存储器使用限制"
        #[cfg(feature = "hpm67-fix")]
        if code == 2 {
            // Illegal instruction
            if andes_riscv::riscv::register::mtval::read() == 0x0 {
                return;
            }
        }

        let trap_frame = &*trap_frame;
        if code < __EXCEPTIONS.len() {
            let h = &__EXCEPTIONS[code];
            if let Some(handler) = h {
                handler(trap_frame);
            } else {
                ExceptionHandler(trap_frame);
            }
        } else {
            ExceptionHandler(trap_frame);
        }
        ExceptionHandler(trap_frame)
    } else if code < __INTERRUPTS.len() {
        let h = &__INTERRUPTS[code];
        if let Some(handler) = h {
            handler();
        } else {
            DefaultHandler();
        }
    } else {
        DefaultHandler();
    }
}

global_asm!(
    r#"
    .section .isr_vector, "ax"
    .global CORE_LOCAL
CORE_LOCAL:
    // save registers
    addi sp, sp, -(16 * 4)
    sw ra, 0(sp)
    sw t0, 4(sp)
    sw t1, 8(sp)
    sw t2, 12(sp)
    sw t3, 16(sp)
    sw t4, 20(sp)
    sw t5, 24(sp)
    sw t6, 28(sp)
    sw a0, 32(sp)
    sw a1, 36(sp)
    sw a2, 40(sp)
    sw a3, 44(sp)
    sw a4, 48(sp)
    sw a5, 52(sp)
    sw a6, 56(sp)
    sw a7, 60(sp)

    add a0, sp, zero
    jal ra, _start_rust_CORE_LOCAL

    // restore registers
    lw ra, 0(sp)
    lw t0, 4(sp)
    lw t1, 8(sp)
    lw t2, 12(sp)
    lw t3, 16(sp)
    lw t4, 20(sp)
    lw t5, 24(sp)
    lw t6, 28(sp)
    lw a0, 32(sp)
    lw a1, 36(sp)
    lw a2, 40(sp)
    lw a3, 44(sp)
    lw a4, 48(sp)
    lw a5, 52(sp)
    lw a6, 56(sp)
    lw a7, 60(sp)
    addi sp, sp, 16 * 4

    mret
    "#,
);
