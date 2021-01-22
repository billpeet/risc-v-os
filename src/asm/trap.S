# trap.S
# Assembly-level trap handler
.option norvc
.altmacro
.set NUM_GP_REGS, 32    # number of registers per context
.set NUM_FP_REGS, 32
.set REG_SIZE, 8        # register size in bytes
.set MAX_CPUS, 8        # max CPUs

# macros for saving/restoring multiple registers
.macro save_gp i, basereg=t6
    sd      x\i, ((\i)*REG_SIZE)(\basereg)
.endm
.macro load_gp i, basereg=t6
    ld      x\i, ((\i)*REG_SIZE)(\basereg)
.endm
.macro save_fp i, basereg=t6
    fsd     f\i, ((NUM_GP_REGS+(\i))*REG_SIZE)(\basereg)
.endm
.macro load_fp i, basereg=t6
    fld     f\i, ((NUM_GP_REGS+(\i))*REG_SIZE)(\basereg)
.endm

.section .text
.global m_trap_vector
.align 4
m_trap_vector:
    # save all registers before we do anything
    csrrw   t6, mscratch, t6    # atomically swaps t6 into mscratch, and the old value of mscratch into t6
    .set i, 1
    .rept 30
        save_gp %i
        .set    i, i+1
    .endr

    # Save t6 register, which was swapped out of mscratch
    mv      t5, t6
    csrr    t6, mscratch
    save_gp 31, t5

    # Restore kernel trap frame into mscratch
    csrw    mscratch, t5

    # Poke everything we need into function parameters and call m_trap
    csrr    a0, mepc
    csrr    a1, mtval
    csrr    a2, mcause
    csrr    a3, mhartid
    csrr    a4, mstatus
    csrr    a5, mscratch
    la      t0, KERNEL_STACK_END
    ld      sp, 0(t0)
    call    m_trap

    # Now returned from m_trap, restore all registers and return
    csrw    mepc, a0        # m_trap will poke return address into a0
    csrr    t6, mscratch    # load trap frame back into t6
    
    # Restore all GP registers
    .set i, 1
    .rept 31
        load_gp %i
        .set    i, i+1
    .endr

    # return to address returned by m_trap
    mret

.global switch_to_user
switch_to_user:
    # a0 - Frame address
    csrw    mscratch, a0

    # program counter
    ld a1, 520(a0)
    # satp
    ld a2, 512(a0)
    # processor mode
    ld a3, 552(a0)

    # bit 7 for machine interrupt enable (MPIE)
    # bit 5 for supervisor interrupt enable (SPIE)
    li      t0, (1 << 7) | (1 << 5) | 1 << 13
    # combine enable bits with mode bits
    slli    a3, a3, 11
    or      t0, t0, a3
    csrw    mstatus, t0
    csrw    mepc, a1
    csrw    satp, a2
    # 0xaaa = enable MEIE/SEIE (external), MTIE/STIE (timer) and MSIE/SSIE (software) interrupts
    li      t1, 0xaaa
    csrw    mie, t1
    la      t2, m_trap_vector   # write trap vector again
    csrw    mtvec, t2
    # sfence.vma                  # force flush the TLB
    mv      t6, a0
    # reload all registers again so we can start running the process
    .set    i, 0
    .rept   32
        load_fp %i
        .set i, i+1
    .endr

    .set    i, 1
    .rept   31
        load_gp %i, t6
        .set i, i + 1
    .endr

    mret

.global make_syscall
make_syscall:
    # using libgloss convention - a7 is system call number, a0-a5 are args
    mv  a7, a0
    mv  a0, a1
    mv  a1, a2
    mv  a2, a3
    mv  a3, a4
    mv  a4, a5
    mv  a5, a6
    ecall
    ret