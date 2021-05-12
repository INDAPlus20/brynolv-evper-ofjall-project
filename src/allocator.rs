use core::{
	alloc::{GlobalAlloc, Layout},
	ptr::NonNull,
};

use bootloader::boot_info::{MemoryRegion, MemoryRegionKind};
use x86_64::{
	addr,
	registers::control::Cr3,
	structures::paging::{PageTable, PageTableFlags, PhysFrame},
	PhysAddr, VirtAddr,
};

const MAX_PHYS_MEM: usize = 32 * 1024 * 1024 * 1024 >> 12;

static mut FRAME_ALLOCATOR: FrameAllocator = FrameAllocator {
	free_frames: [0xFF; MAX_PHYS_MEM / 8],
	first_free_frame: usize::MAX,
	last_free_frame: 0,
};
static mut MEMORY_MAPPER: MemoryMapper = MemoryMapper { pml4t_ptr: 0 as _ };
#[global_allocator]
static mut MEMORY_ALLOCATOR: MemoryAllocator = MemoryAllocator {
	first_block: 0 as _,
};

struct FrameAllocator {
	free_frames: [u8; MAX_PHYS_MEM / 8],
	first_free_frame: usize,
	last_free_frame: usize,
}

impl FrameAllocator {
	fn initialize(&mut self, mem: &[MemoryRegion]) {
		for mem in mem {
			// println!("{}: {:x}..{:x}", match mem.kind {
			//     MemoryRegionKind::Usable => "Usable",
			//     MemoryRegionKind::Bootloader => "Bootloader",
			//     MemoryRegionKind::UnknownUefi(_) => "Unkown Uefi",
			//     MemoryRegionKind::UnknownBios(_) => "Unknown Bios",
			//     _ => "Unknown"
			// }, mem.start, mem.end);

			match mem.kind {
				MemoryRegionKind::Usable => {
					let first_frame = mem.start >> 12;
					let last_frame = mem.end - 1 >> 12;

					for frame in first_frame..=last_frame {
						self.set_unused(frame as _);
					}
				}
				_ => {}
			}
		}
	}

	fn set_used(&mut self, index: usize) {
		let b = index / 8;
		let r = index % 8;
		self.free_frames[b] |= 1 << r;
		if index == self.first_free_frame {
			for _ in self.first_free_frame..=self.last_free_frame {
				self.first_free_frame += 1;
				if !self.get(self.first_free_frame) {
					break;
				}
			}
		}
		if index == self.last_free_frame {
			for _ in self.first_free_frame..=self.last_free_frame {
				self.last_free_frame = match self.last_free_frame.checked_sub(1) {
					Some(v) => v,
					None => break,
				};
				if !self.get(self.last_free_frame) {
					break;
				}
			}
		}
	}

	fn set_unused(&mut self, index: usize) {
		let b = index / 8;
		let r = index % 8;
		self.free_frames[b] &= !(1 << r);
		if index < self.first_free_frame {
			self.first_free_frame = index;
		}
		if index > self.last_free_frame {
			self.last_free_frame = index;
		}
	}

	fn get(&self, index: usize) -> bool {
		self.free_frames[index / 8] & 1 << index % 8 > 0
	}

	fn allocate_frame(&mut self) -> PhysFrame {
		let frame =
			PhysFrame::from_start_address(PhysAddr::new((self.first_free_frame as u64) << 12)).unwrap();
		self.set_used(self.first_free_frame);
		frame
	}

	fn free_frame(&mut self, frame: PhysFrame) {
		self.set_unused(frame.start_address().as_u64() as usize >> 12);
	}
}

struct MemoryMapper {
	pml4t_ptr: *mut PageTable,
}

impl MemoryMapper {
	unsafe fn initialize(&mut self) {
		self.pml4t_ptr = phys_to_virt(Cr3::read().0.start_address()).as_mut_ptr();
	}

	unsafe fn is_mapped(&self, virt: VirtAddr) -> bool {
		let addr = virt.as_u64();
		let idx4 = addr as usize >> 39 & 0x1FF;
		let idx3 = addr as usize >> 30 & 0x1FF;
		let idx2 = addr as usize >> 21 & 0x1FF;
		let idx1 = addr as usize >> 12 & 0x1FF;

		let pml4t = &mut *self.pml4t_ptr;
		if pml4t[idx4].is_unused() {
			return false;
		}

		let pdpt: &mut PageTable = &mut *phys_to_virt(pml4t[idx4].addr()).as_mut_ptr();
		if pdpt[idx3].is_unused() {
			return false;
		} else if pdpt[idx3].flags().contains(PageTableFlags::HUGE_PAGE) {
			return true;
		}

		let pdt: &mut PageTable = &mut *phys_to_virt(pdpt[idx3].addr()).as_mut_ptr();
		if pdt[idx2].is_unused() {
			return false;
		} else if pdt[idx2].flags().contains(PageTableFlags::HUGE_PAGE) {
			return true;
		}

		let pt: &mut PageTable = &mut *phys_to_virt(pdt[idx2].addr()).as_mut_ptr();
		if pt[idx1].is_unused() { false } else { true }
	}

	unsafe fn map(&mut self, virt: VirtAddr, frame: PhysFrame) {
		let addr = virt.as_u64();
		let idx4 = addr as usize >> 39 & 0x1FF;
		let idx3 = addr as usize >> 30 & 0x1FF;
		let idx2 = addr as usize >> 21 & 0x1FF;
		let idx1 = addr as usize >> 12 & 0x1FF;

		let pml4t = &mut *self.pml4t_ptr;
		if pml4t[idx4].is_unused() {
			let pdpt_frame = FRAME_ALLOCATOR.allocate_frame();
			let pdpt_ptr: *mut PageTable = phys_to_virt(pdpt_frame.start_address()).as_mut_ptr();
			*pdpt_ptr = PageTable::new();
			pml4t[idx4].set_addr(
				pdpt_frame.start_address(),
				PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
			);
		}

		let pdpt: &mut PageTable = &mut *phys_to_virt(pml4t[idx4].addr()).as_mut_ptr();
		if pdpt[idx3].is_unused() {
			let pdt_frame = FRAME_ALLOCATOR.allocate_frame();
			let pdt_ptr: *mut PageTable = phys_to_virt(pdt_frame.start_address()).as_mut_ptr();
			*pdt_ptr = PageTable::new();
			pdpt[idx3].set_addr(
				pdt_frame.start_address(),
				PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
			);
		} else if pdpt[idx3].flags().contains(PageTableFlags::HUGE_PAGE) {
			panic!("Cannot map already mapped page");
		}

		let pdt: &mut PageTable = &mut *phys_to_virt(pdpt[idx3].addr()).as_mut_ptr();
		if pdt[idx2].is_unused() {
			let pt_frame = FRAME_ALLOCATOR.allocate_frame();
			let pt_ptr: *mut PageTable = phys_to_virt(pt_frame.start_address()).as_mut_ptr();
			*pt_ptr = PageTable::new();
			pdt[idx2].set_addr(
				pt_frame.start_address(),
				PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
			);
		} else if pdt[idx2].flags().contains(PageTableFlags::HUGE_PAGE) {
			panic!("Cannot map already mapped page");
		}

		let pt: &mut PageTable = &mut *phys_to_virt(pdt[idx2].addr()).as_mut_ptr();
		if pt[idx1].is_unused() {
			pt[idx1].set_frame(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
		} else {
			panic!("Cannot map already mapped page");
		}
	}

	unsafe fn unmap(&mut self, virt: VirtAddr) {
		let addr = virt.as_u64();
		let idx4 = addr as usize >> 39 & 0x1FF;
		let idx3 = addr as usize >> 30 & 0x1FF;
		let idx2 = addr as usize >> 21 & 0x1FF;
		let idx1 = addr as usize >> 12 & 0x1FF;

		let pml4t = &mut *self.pml4t_ptr;
		if pml4t[idx4].is_unused() {
			panic!("Page is not mapped");
		} else {
			let pdpt: &mut PageTable = &mut *phys_to_virt(pml4t[idx4].addr()).as_mut_ptr();
			if pdpt[idx3].is_unused() {
				panic!("Page is not mapped");
			} else if pdpt[idx3].flags().contains(PageTableFlags::HUGE_PAGE) {
				let pdt: &mut PageTable = &mut *phys_to_virt(pdpt[idx3].addr()).as_mut_ptr();
				if pdt[idx2].is_unused() {
					panic!("Page is not mapped");
				} else if pdt[idx2].flags().contains(PageTableFlags::HUGE_PAGE) {
					let pt: &mut PageTable = &mut *phys_to_virt(pdt[idx2].addr()).as_mut_ptr();
					if pt[idx1].is_unused() {
						panic!("Page is not mapped");
					} else {
						let frame = pt[idx1].frame().unwrap();
						pt[idx1].set_unused();
						FRAME_ALLOCATOR.free_frame(frame);

						for i in 0..512 {
							if !pt[i].is_unused() {
								return;
							}
						}

						pdt[idx2].set_unused();
					}

					for i in 0..512 {
						if !pdt[i].is_unused() {
							return;
						}
					}

					pdpt[idx3].set_unused();
				}

				for i in 0..512 {
					if !pdpt[i].is_unused() {
						return;
					}
				}

				pml4t[idx4].set_unused();
			}
		}
	}
}

struct MemoryAllocator {
	first_block: *mut MemoryBlock,
}

impl MemoryAllocator {
	unsafe fn initialize(&mut self, start_addr: u64) {
		if !MEMORY_MAPPER.is_mapped(VirtAddr::new(start_addr)) {
			let frame = FRAME_ALLOCATOR.allocate_frame();
			MEMORY_MAPPER.map(VirtAddr::new(start_addr), frame);
		}
		self.first_block = start_addr as _;
		self.first_block.write(MemoryBlock {
			previous: None,
			next: None,
			layout: Layout::new::<()>(),
			data: align_up(
				start_addr + core::mem::size_of::<MemoryBlock>() as u64,
				core::mem::align_of::<()>() as _,
			),
		});
	}
}

unsafe impl GlobalAlloc for MemoryAllocator {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		x86_64::instructions::interrupts::without_interrupts(|| {
			let mut current_block = self.first_block;

			while let Some(mut next) = (*current_block).next {
				let block_between_addr = align_up(
					(*current_block).end_of_data_addr(),
					core::mem::align_of::<MemoryBlock>() as _,
				);
				let data_after_block_addr = align_up(
					block_between_addr + core::mem::size_of::<MemoryBlock>() as u64,
					layout.align() as _,
				);
				if next.as_ptr() as u64 > data_after_block_addr + layout.size() as u64 {
					let new_block = (*current_block).spawn_block(layout, Some(next));
					let addr = new_block.as_ref().data as _;
					return addr;
				}
				current_block = next.as_ptr();
			}

			let addr = (*current_block).spawn_block(layout, None).as_ref().data as _;
			addr
		})
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		x86_64::instructions::interrupts::without_interrupts(|| {
			let block_addr = align_down(
				ptr as u64 - core::mem::size_of::<MemoryBlock>() as u64,
				core::mem::align_of::<MemoryBlock>() as _,
			);
			let block = (block_addr as *mut MemoryBlock).as_mut().unwrap();
			let cur_max_addr = block.end_of_data_addr() - 1;
			let cur_max_page = cur_max_addr >> 12;
			let cur_min_addr = block as *const _ as u64;
			let cur_min_page = cur_min_addr >> 12;
			let prev = block.previous.unwrap();
			let prev_max_addr = prev.as_ref().end_of_data_addr() - 1;
			let prev_max_page = prev_max_addr >> 12;
			let next_min_page = if let Some(next) = block.next {
				let next_min_addr = next.as_ptr() as u64;
				next_min_addr >> 12
			} else {
				u64::MAX
			};

			let min_page_to_unmap = (prev_max_page + 1).max(cur_min_page);
			let max_page_to_unwrap = cur_max_page.min(next_min_page - 1);
			for page in min_page_to_unmap..=max_page_to_unwrap {
				MEMORY_MAPPER.unmap(VirtAddr::new(page << 12));
			}

			block.previous.unwrap().as_mut().next = block.next;
			if let Some(mut next) = block.next {
				next.as_mut().previous = block.previous;
			}
		})
	}
}

struct MemoryBlock {
	previous: Option<NonNull<MemoryBlock>>,
	next: Option<NonNull<MemoryBlock>>,
	layout: Layout,
	data: u64,
}

impl MemoryBlock {
	unsafe fn data_ptr<T>(&self) -> *mut T {
		self.data as _
	}

	unsafe fn end_of_data_addr(&self) -> u64 {
		self.data + self.layout.size() as u64
	}

	unsafe fn self_addr(&self) -> u64 {
		self as *const _ as _
	}

	unsafe fn spawn_block(
		&mut self,
		layout: Layout,
		next: Option<NonNull<MemoryBlock>>,
	) -> NonNull<MemoryBlock> {
		let block_addr = align_up(self.end_of_data_addr(), core::mem::align_of::<Self>() as _);
		let data_addr = align_up(
			block_addr + core::mem::size_of::<Self>() as u64,
			layout.align() as _,
		);
		let block_addr = align_down(
			data_addr - core::mem::size_of::<Self>() as u64,
			core::mem::align_of::<Self>() as _,
		);
		let first_page = block_addr >> 12;
		let last_page = data_addr + layout.size() as u64 - 1 >> 12;

		for page in first_page..=last_page {
			let addr = VirtAddr::new(page << 12);
			if !MEMORY_MAPPER.is_mapped(addr) {
				let frame = FRAME_ALLOCATOR.allocate_frame();
				MEMORY_MAPPER.map(addr, frame);
			}
		}

		(block_addr as *mut MemoryBlock).write(MemoryBlock {
			previous: Some(NonNull::new_unchecked(self as _)),
			next,
			layout,
			data: data_addr,
		});
		let ptr = NonNull::new_unchecked(block_addr as _);
		self.next = Some(ptr);
		if let Some(mut next) = next {
			next.as_mut().previous = Some(ptr);
		}
		ptr
	}
}

pub unsafe fn initialize(mem: &[MemoryRegion]) {
	FRAME_ALLOCATOR.initialize(mem);
	FRAME_ALLOCATOR.set_used(0);
	MEMORY_MAPPER.initialize();
	MEMORY_ALLOCATOR.initialize(0xFFFF_F000_0000_0000);
}

const PHYS_MAP_START: u64 = 0xFFFF_FF80_0000_0000;

fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
	VirtAddr::new(phys.as_u64() | PHYS_MAP_START)
}

fn align_down(addr: u64, align: u64) -> u64 {
	addr - addr % align
}

fn align_up(addr: u64, align: u64) -> u64 {
	let rest = addr % align;
	if rest == 0 { addr } else { addr + align - rest }
}
