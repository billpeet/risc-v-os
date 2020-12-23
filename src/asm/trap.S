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
    fsd     f\i, ((NUM_GP_REGS+(\i)*REG_SIZE)(\basereg)
.endm

.macro load_fp i, basereg=t6
    fld     f\i, ((NUM_GP_REGS+(\i)*REG_SIZE)(\basereg)
.endm

.global m_trap_vector
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
    mv      a5, t5
    ld      sp, 520(a5)
    call    m_trap

    # Now returned from m_trap, restore all register and return
    csrw    mepc, a0        # m_trap will poke return address into a0
    csrr    t6, mscratch    # load trap frame back into t6
    
    # Restore all GP registers
    .set i, 1
    .rept 30
        load_gp %i
        .set    i, i+1
    .endr

    mret

.global make_syscall
make_syscall:
    ecall
    ret