# boot.S
# bootloader for SoS

# Disable generation of compressed instructions
.option norvc

.section .text.init
.global _start
_start:
    # Disable linker instruction relaxation
.option push
.option norelax
    la      gp, _global_pointer
.option pop
    # SATP should be zero, but let's make sure
    csrw    satp, zero
    # Any hardware threads (hart) that are not bootstrapping need to wait for an IPI
    csrr    t0, mhartid
    bnez    t0, 3f

# Clear BSS - the BSS section is expected to be zero
clear_bss:
    la      a0, _bss_start
    la      a1, _bss_end
    bgeu    a0, a1, 2f
1:
    sd      zero, (a0)
    addi    a0, a0, 8
    bltu    a0, a1, 1b

2:
# Control registers, set the stack, status, mepc and mtvec to return to the main function
#    li      t5, 0xffff
#    csrw    medeleg, t5
#    csrw    mideleg, t5
    la      sp, _stack_end                          # init stack pointer
    li      t0, (0b11 << 11)                        # bits [12:11] set for machine mode
    csrw    mstatus, t0
    la      t1, kinit                               # machine mode Rust entry point
    csrw    mepc, t1
    la      t2, asm_trap_vector                     # trap vector
    csrw    mtvec, t2
    la      ra, 2f                                  # return address from Rust code
# We use mret here so that the mstatus register is properly updated
    mret

2:
    # Will return here after kinit is complete
    li      t0, (1 << 8) | (1 << 5)                 # set sstatus register - bit 8 for previous protection mode, bit 5 for previous interrut-enable
    csrw    sstatus, t0
    la      t1, kmain                               # supervisor mode Rust entry point
    csrw    sepc, t1
    li      t2, (1 << 1) | (1 << 5) | (1 << 9)      # set machine interrupt delegate (mideleg) register - software, timer and external interrupts delegated to supervisor mode
    csrw    mideleg, t2
    csrw    sie, t2                                 # set supervisor interrupt enable (sie) register - enable software, timer and external interrupts in supervisor mode
    la      t3, asm_trap_vector                     # set supervisor trap vector (stvec)
    csrw    stvec, t3
    csrw    satp, a0                                # set supervisor address translation and protection register (satp)
    sfence.vma                                      # force CPU to take our satp register
    sret                                            # sret will put us in supervisor mode and re-enable interrupts

3:
# Parked harts go here

# Infinite loop
4:
    wfi
    j       4b