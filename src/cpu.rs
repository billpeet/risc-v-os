use alloc::{format, string::String};

#[repr(usize)]
pub enum SatpMode {
	Off = 0,
	Sv39 = 8,
	Sv48 = 9,
}

#[repr(usize)]
pub enum CpuMode {
	User = 0,
	Supervisor = 1,
	Machine = 3,
}

#[repr(usize)]
pub enum Registers {
	Zero = 0,
	Ra,
	Sp,
	Gp,
	Tp,
	T0,
	T1,
	T2,
	S0,
	S1,
	A0, /* 10 */
	A1,
	A2,
	A3,
	A4,
	A5,
	A6,
	A7,
	S2,
	S3,
	S4, /* 20 */
	S5,
	S6,
	S7,
	S8,
	S9,
	S10,
	S11,
	T3,
	T4,
	T5, /* 30 */
	T6,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapFrame {
	pub regs: [usize; 32],  // 0 - 255
	pub fregs: [usize; 32], // 256 - 511
	pub satp: usize,        // 512 - 519
	pub pc: usize,          // 520
	pub hartid: usize,      // 528
	pub qm: usize,          // 536
	pub pid: usize,         // 544
	pub mode: usize,        // 552
}

impl TrapFrame {
	pub const fn zero() -> Self {
		TrapFrame {
			regs: [0; 32],
			fregs: [0; 32],
			satp: 0,
			pc: 0,
			hartid: 0,
			qm: 1,
			pid: 0,
			mode: 0,
		}
	}
}

pub static mut KERNEL_TRAP_FRAME: [TrapFrame; 8] = [TrapFrame::zero(); 8];

pub fn wfi() {
	unsafe {
		asm!("wfi");
	}
}

pub const fn build_satp(mode: SatpMode, asid: usize, addr: usize) -> usize {
	(mode as usize) << 60 | (asid & 0xffff) << 44 | (addr >> 12) & 0xff_ffff_ffff
}

pub fn mhartid_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr $0, mhartid" :"=r"(rval));
		rval
	}
}

pub fn mstatus_write(val: usize) {
	unsafe {
		llvm_asm!("csrw	mstatus, $0" ::"r"(val));
	}
}

pub fn mstatus_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr	$0, mstatus":"=r"(rval));
		rval
	}
}

pub fn stvec_write(val: usize) {
	unsafe {
		llvm_asm!("csrw	stvec, $0" ::"r"(val));
	}
}

pub fn stvec_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr	$0, stvec" :"=r"(rval));
		rval
	}
}

pub fn mscratch_write(val: usize) {
	unsafe {
		llvm_asm!("csrw	mscratch, $0" ::"r"(val));
	}
}

pub fn mscratch_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr	$0, mscratch" : "=r"(rval));
		rval
	}
}

pub fn mscratch_swap(to: usize) -> usize {
	unsafe {
		let from;
		llvm_asm!("csrrw	$0, mscratch, $1" : "=r"(from) : "r"(to));
		from
	}
}

pub fn sscratch_write(val: usize) {
	unsafe {
		llvm_asm!("csrw	sscratch, $0" ::"r"(val));
	}
}

pub fn sscratch_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr	$0, sscratch" : "=r"(rval));
		rval
	}
}

pub fn sscratch_swap(to: usize) -> usize {
	unsafe {
		let from;
		llvm_asm!("csrrw	$0, sscratch, $1" : "=r"(from) : "r"(to));
		from
	}
}

pub fn sepc_write(val: usize) {
	unsafe {
		llvm_asm!("csrw sepc, $0" :: "r"(val));
	}
}

pub fn sepc_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr $0, sepc" :"=r"(rval));
		rval
	}
}

pub fn satp_write(val: usize) {
	unsafe {
		llvm_asm!("csrw satp, $0" :: "r"(val));
	}
}

pub fn satp_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr $0, satp" :"=r"(rval));
		rval
	}
}

pub fn satp_fence(vaddr: usize, asid: usize) {
	unsafe {
		llvm_asm!("sfence.vma $0, $1" :: "r"(vaddr), "r"(asid));
	}
}

pub fn satp_fence_asid(asid: usize) {
	unsafe {
		llvm_asm!("sfence.vma zero, $0" :: "r"(asid));
	}
}

pub const MTIMER_TICKS_PER_MS: u64 = 10_000;
pub const MTIMER_TICKS_PER_SEC: u64 = MTIMER_TICKS_PER_MS * 1000;
pub const MTIMER_TICKS_PER_MIN: u64 = MTIMER_TICKS_PER_SEC * 60;
pub const MTIMER_TICKS_PER_HOUR: u64 = MTIMER_TICKS_PER_MIN * 60;

#[derive(Copy, Clone)]
pub struct MachineTime {
	pub ticks: u64,
}

impl MachineTime {
	pub fn from_ticks(ticks: u64) -> Self {
		MachineTime { ticks }
	}

	pub fn from_ms(ms: u64) -> Self {
		MachineTime {
			ticks: ms * MTIMER_TICKS_PER_MS,
		}
	}

	pub fn zero() -> Self {
		MachineTime { ticks: 0 }
	}

	pub fn as_u64(&self) -> u64 {
		self.ticks
	}

	pub fn offset(&self, offset: MachineTime) -> Self {
		MachineTime {
			ticks: self.ticks + offset.as_u64(),
		}
	}

	pub fn offset_ticks(&self, ticks: u64) -> Self {
		MachineTime {
			ticks: self.ticks + ticks,
		}
	}

	pub fn offset_ms(&self, ms: u64) -> Self {
		MachineTime {
			ticks: self.ticks + ms * MTIMER_TICKS_PER_MS,
		}
	}
	pub fn formatted(&self) -> String {
		let min = self.ticks / MTIMER_TICKS_PER_MIN;
		let total_sec = self.ticks % MTIMER_TICKS_PER_MIN;
		let sec = total_sec / MTIMER_TICKS_PER_SEC;
		let ms = (total_sec % MTIMER_TICKS_PER_SEC) / MTIMER_TICKS_PER_MS;
		format!("{}:{}.{}", min, sec, ms)
	}
}

pub fn get_mtime() -> MachineTime {
	let mtime = 0x0200_bff8 as *const u64;
	let mtime_u64;
	unsafe { mtime_u64 = mtime.read_volatile() }
	MachineTime::from_ticks(mtime_u64)
}

pub fn set_next_minterrupt(next_time: MachineTime) {
	let mtimecmp = 0x0200_4000 as *mut u64;
	unsafe {
		mtimecmp.write_volatile(next_time.as_u64());
	}
}

pub unsafe fn memcpy(dest: *mut u8, src: *const u8, bytes: usize) {
	let bytes_as_8 = bytes / 8;
	let dest_as_8 = dest as *mut u64;
	let src_as_8 = src as *const u64;

	for i in 0..bytes_as_8 {
		*dest_as_8.add(i) = *src_as_8.add(i);
	}

	let bytes_completed = bytes_as_8 * 8;
	let bytes_remaining = bytes - bytes_completed;
	for i in bytes_completed..bytes_remaining {
		*dest_as_8.add(i) = *src_as_8.add(i);
	}
}
