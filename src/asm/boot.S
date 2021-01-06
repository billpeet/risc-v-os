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

    la      a0, _bss_start                          # Clear BSS - the BSS section is expected to be zero
    la      a1, _bss_end
    bgeu    a0, a1, 2f
1:
    sd      zero, (a0)
    addi    a0, a0, 8
    bltu    a0, a1, 1b

2:
    # Control registers, set the stack, status, mepc and mtvec to return to the main function
    la      sp, _stack_end                          # init stack pointer
    li      t0, (0b11 << 11)                        # bits [12:11] set for machine mode
    csrw    mstatus, t0
    csrw    mie, zero                               # disable all interrupts
    la      t1, kinit                               # machine mode Rust entry point
    csrw    mepc, t1
    la      ra, 2f                                  # return address from Rust code
    mret                                            # We use mret here so that the mstatus register is properly updated

2:
    # Will return here after kinit is complete
    li      t0, (0b01 << 11) | (1 << 7) | (1 << 5)  # set mstatus register - bit 8 for previous protection mode, bit 5 for previous interrut-enable
    csrw    mstatus, t0
    la      t2, m_trap_vector                       # set machine trap vector (mtvec)
    csrw    mtvec, t2
    # la      t3, asm_trap_vector                   # set supervisor trap vector (stvec)
    # csrw    stvec, t3
    la      t1, kmain                               # supervisor mode Rust entry point
    csrw    mepc, t1
    li      t2, (1 << 1) | (1 << 5) | (1 << 9)      # set machine interrupt delegate (mideleg) register - software, timer and external interrupts delegated to supervisor mode
    csrw    mie, t2                                 # set machine interrupt enable (sie) register - enable software, timer and external interrupts in supervisor mode
    mret                                            # use mret so mstatus gets set

3:
	# Parked harts go here. We need to set these
	# to only awaken if it receives a software interrupt,
	# which we're going to call the SIPI (Software Intra-Processor Interrupt).
	# We call the SIPI by writing the software interrupt into the Core Local Interruptor (CLINT)
	# Which is calculated by: base_address + hart * 4
	# where base address is 0x0200_0000 (MMIO CLINT base address)
	# We only use additional harts to run user-space programs, although this may
	# change.

	# We divide up the stack so the harts aren't clobbering one another.
	la		sp, _stack_end
	li		t0, 0x10000
	csrr	a0, mhartid
	mul		t0, t0, a0
	sub		sp, sp, t0

	# The parked harts will be put into machine mode with interrupts enabled.
	li		t0, 0b11 << 11 | (1 << 7)
	csrw	mstatus, t0
	# Allow for MSIP (Software interrupt). We will write the MSIP from hart #0 to
	# awaken these parked harts.
	li		t3, (1 << 3)
	csrw	mie, t3
	# Machine's exception program counter (MEPC) is set to the Rust initialization
	# code and waiting loop.
	la		t1, kinit_hart
	csrw	mepc, t1
	# Machine's trap vector base address is set to `m_trap_vector`, for
	# "machine" trap vector. The Rust initialization routines will give each
	# hart its own trap frame. We can use the same trap function and distinguish
	# between each hart by looking at the trap frame.
	la		t2, m_trap_vector
	la		t2, m_trap_vector
	csrw	mtvec, t2
	# Whenever our hart is done initializing, we want it to return to the waiting
	# loop, which is just below mret.
	la		ra, 4f
	# We use mret here so that the mstatus register is properly updated.
	mret

# Infinite loop
4:
    wfi
    j       4b