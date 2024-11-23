# hpm-riscv-rt

Minimal startup/runtime for RISC-V CPUs from HPMicro.

Much of the code in this package originated in the [rust-embedded/riscv] repository.

[rust-embedded/riscv]: https://github.com/rust-embedded/riscv

## How to use

Create the `memory.x` linker script file, then add link args.

```ld

MEMORY
{
    XPI0_HEADER : ORIGIN = 0x80000000, LENGTH = 0x3000 /* bootheader */
    XPI0_APP    : ORIGIN = 0x80003000, LENGTH = 1024K - 0x3000 /* app firmware */

    ILM        : ORIGIN = 0x00000000, LENGTH =  256K /* instruction local memory */
    DLM        : ORIGIN = 0x00080000, LENGTH =  256K /* data local memory */

    AXI_SRAM    : ORIGIN = 0x01080000, LENGTH = 1M
    AHB_SRAM    : ORIGIN = 0xF0300000, LENGTH = 32K
    APB_SRAM    : ORIGIN = 0xF40F0000, LENGTH = 8K

    SDRAM       : ORIGIN = 0x40000000, LENGTH = 32M
}


REGION_ALIAS("REGION_TEXT", XPI0_APP);
REGION_ALIAS("REGION_FASTTEXT", ILM);
REGION_ALIAS("REGION_FASTDATA", DLM);
REGION_ALIAS("REGION_RODATA", XPI0_APP);
REGION_ALIAS("REGION_DATA", DLM);
REGION_ALIAS("REGION_BSS", DLM);
REGION_ALIAS("REGION_HEAP", DLM);
REGION_ALIAS("REGION_STACK", DLM);
REGION_ALIAS("REGION_NONCACHEABLE_RAM", DLM);
```

## Re-exported macros

<!-- intro to entry, fast, interrupt, pre_init >

### `entry!`

```rust
#[entry]
fn main() -> ! {
    loop {
        // your code here
    }
}
```

Marks a function as the entry point of the program.

### `fast!`

```rust
#[fast]
fn fast_handler() {
    // your code here
}

#[fast]
static mut FAST_DATA: u32 = 0;

#[fast]
static FAST_DATA_INIT: MaybeUninit<u32> = MaybeUninit::uninit();
```

Marks a function as a fast interrupt handler, or a static as fast data.

### `interrupt!`

```rustp
#[interrupt]
fn GPIO0() {
    // your code here
}

#[interrupt(MachineTimer)]
fn timer() {
    // your code here
}
```

Marks a function as an interrupt handler, both core local and external.

### `pre_init!`

```rust
#[pre_init]
fn before_main() {
    // your code here
}
```

Marks a function that will be executed before `main`. Useful for setting up the environment(SDRAM, etc).
