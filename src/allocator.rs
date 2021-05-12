//! Dynamic memory allocation.
//!
//! This module contains structures used for allocating physical
//! and virtual memory and handling the mappings between them.
//! The bootloader maps all of physical memory to virtual memory, starting at
//! address `0xFFFFFF8000000000`.
//!
//! # Physical memory allocation
//!
//! Physical memory allocation is handled by the [FRAME_ALLOCATOR] static.
//! It uses a simple bitmap, where each bit is set if
//! the corresponding frame is either used or unusable, and reset if not.
//! Every frame not explicitly marked usable by the bootloader is
//! assumed unusable.
//!
//! [FRAME_ALLOCATOR] does not currently check if there is any unused physical memory;
//! trying to allocate memory when there is none available may return an already allocated
//! frame, which may result in undefined behaviour.
//!
//! # Virtual memory allocation
//!
//! Virtual memory allocation is handled by the [MEMORY_ALLOCATOR] static.
//! It uses a linked list of [MemoryBlock]s to keep track of allocated memory.
//! Every allocated block of memory is preceded by a [MemoryBlock] node.
//! These nodes are placed as close as possible to the allocated memory while upholding
//! all alignment requirements. As such, it is possible to retreive a [MemoryBlock] node
//! from the pointer to the allocated memory.
//!
//! The heap starts at the address `0xFFFFF00000000000`.
//!
//! Using one [MemoryBlock] for every allocation is not optimal; every allocation will get some overhead. Many small allocations
//! will use much more memory than a few large ones.
//!
//! # Virtual memory mapping
//!
//! Virtual memory mapping is handled by the [MEMORY_MAPPER] static.
//! Mapping an address may allocate additional physical frames as needed.

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

/// The maximum supported physical memory, in physical frames.
///
/// Currently, this is 32GiB.
const MAX_PHYS_MEM: usize = 32 * 1024 * 1024 * 1024 >> 12;

/// Handles physical memory allocation.
///
/// Every frame starts out as unusable.
/// Must be initialized before use.
static mut FRAME_ALLOCATOR: FrameAllocator = FrameAllocator {
	free_frames: [0xFF; MAX_PHYS_MEM / 8],
	first_free_frame: usize::MAX,
	last_free_frame: 0,
};

/// Handles mapping virtual memory to physical memory.
///
/// Must be initialized before use.
static mut MEMORY_MAPPER: MemoryMapper = MemoryMapper { pml4t_ptr: 0 as _ };

/// Handles virtual memory allocation.
///
/// Must be initialized before use.
#[global_allocator]
static mut MEMORY_ALLOCATOR: MemoryAllocator = MemoryAllocator {
	first_block: 0 as _,
};

/// A physical frame allocator.
///
/// Contains a simple bitmap which is used to keep track of allocated/freed
/// frames. The first and last free frames are kept track of, to speed up
/// allocations and deallocations.
struct FrameAllocator {
	free_frames: [u8; MAX_PHYS_MEM / 8],
	first_free_frame: usize,
	last_free_frame: usize,
}

impl FrameAllocator {
	/// Initializes the [FrameAllocator].
	///
	/// Every [MemoryRegion] whose [kind] is [`Usable`] will be marked
	/// as unused. Everything else will not be touched, and will remain
	/// in the state it was before the call to [`Self::initialize`].
	///
	/// [kind]: MemoryRegionKind
	/// [`Usable`]: MemoryRegionKind#variant.Usable
	fn initialize(&mut self, mem: &[MemoryRegion]) {
		for mem in mem {
			match mem.kind {
				MemoryRegionKind::Usable => {
					// Every frame must be marked individually

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

	/// Marks a frame as used.
	///
	/// Also updates [`Self::first_free_frame`] and [`Self::last_free_frame`].
	///
	/// # Panics
	///
	/// Panics if `index` is above or equal to [MAX_PHYS_MEM].
	/// May also panic if the frame index given is the last available frame.
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

	/// Marks a frame as unused.
	///
	/// Also updates [`Self::first_free_frame`] and [`Self::last_free_frame`].
	///
	/// # Panics
	///
	/// Panics if `index` is above or equal to [MAX_PHYS_MEM].
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

	/// Checks if the given frame index is marked as unused.
	///
	/// # Panics
	///
	/// Panics if `index` is above or equal to [MAX_PHYS_MEM].
	fn get(&self, index: usize) -> bool {
		self.free_frames[index / 8] & 1 << index % 8 > 0
	}

	/// Allocates a physical frame.
	///
	/// If there are no free frames available,
	/// an already allocated frame may be returned.
	fn allocate_frame(&mut self) -> PhysFrame {
		let frame =
			PhysFrame::from_start_address(PhysAddr::new((self.first_free_frame as u64) << 12)).unwrap();
		self.set_used(self.first_free_frame);
		frame
	}

	/// Frees a physical frame.
	///
	/// # Panics
	///
	/// Panics if the frame is outside of max supported physical memory.
	fn free_frame(&mut self, frame: PhysFrame) {
		self.set_unused(frame.start_address().as_u64() as usize >> 12);
	}
}

/// A virtual to physical memory mapper.
///
/// Only supports allocating 4KiB pages, but can detect and free
/// 2MiB and 1GiB pages.
struct MemoryMapper {
	pml4t_ptr: *mut PageTable,
}

impl MemoryMapper {
	/// Initializes the [MemoryMapper].
	///
	/// Uses the [PageTable] the bootloader already has set up.
	///
	/// # Safety
	///
	/// - `FRAME_ALLOCATOR.initialize(..)` must have been called
	/// - **Only one** [MemoryMapper] may be initialized at the same time
	/// - The `cr3` register must contain the address of a valid [PageTable]
	unsafe fn initialize(&mut self) {
		// Cr3 contains the physical address of the page table,
		// but we need a virtual address.
		self.pml4t_ptr = phys_to_virt(Cr3::read().0.start_address()).as_mut_ptr();
	}

	/// Checks if the given virtual address is currently mapped.
	///
	/// # Safety
	///
	/// [`Self::initialize`] must have been called.
	unsafe fn is_mapped(&self, virt: VirtAddr) -> bool {
		let (idx4, idx3, idx2, idx1) = get_page_table_indices(virt);

		let pml4t = &mut *self.pml4t_ptr;
		if pml4t[idx4].is_unused() {
			return false;
		}

		// As in Self::intialize, we must convert physical addresses to virtual ones.
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

	/// Maps the given virtual address to the given physical frame.
	///
	/// # Panics
	///
	/// Panics if `virt` is already mapped.
	///
	/// # Safety
	///
	/// - [`Self::initialize`] must have been called.
	/// - `frame` must not already be mapped to another virtual address.
	unsafe fn map(&mut self, virt: VirtAddr, frame: PhysFrame) {
		let (idx4, idx3, idx2, idx1) = get_page_table_indices(virt);

		let pml4t = &mut *self.pml4t_ptr;
		if pml4t[idx4].is_unused() {
			// We need to allocate a new page table
			let pdpt_frame = FRAME_ALLOCATOR.allocate_frame();
			let pdpt_ptr: *mut PageTable = phys_to_virt(pdpt_frame.start_address()).as_mut_ptr();
			pdpt_ptr.write(PageTable::new());
			pml4t[idx4].set_addr(
				pdpt_frame.start_address(),
				PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
			);
		}

		let pdpt: &mut PageTable = &mut *phys_to_virt(pml4t[idx4].addr()).as_mut_ptr();
		if pdpt[idx3].is_unused() {
			let pdt_frame = FRAME_ALLOCATOR.allocate_frame();
			let pdt_ptr: *mut PageTable = phys_to_virt(pdt_frame.start_address()).as_mut_ptr();
			pdt_ptr.write(PageTable::new());
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
			pt_ptr.write(PageTable::new());
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

	/// Unmaps the given virtual address and frees the physical frame it was mapped to.
	///
	/// # Panics
	///
	/// Panics if `virt` is not mapped to a physical frame.
	///
	/// # Safety
	///
	/// The caller much make sure that nothing that lies in this page
	/// is read or written to after this call, or that a frame is mapped
	/// to this page before any such call.
	/// The caller much also make sure that any frame that is mapped to this
	/// page doesn't break any of Rust's memory guarantees.
	unsafe fn unmap(&mut self, virt: VirtAddr) {
		let (idx4, idx3, idx2, idx1) = get_page_table_indices(virt);

		// If we have unmapped all entries in a page table,
		// we can unmap the page table itself.
		// This requires the nested abomination below
		// (or some more sofisticated refactoring),
		// as finding a huge page is a kind of early return,
		// while we still want to check if we can unmap page tables.

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

/// Transforms a virtual table into page table indices.
fn get_page_table_indices(virt: VirtAddr) -> (usize, usize, usize, usize) {
	let addr = virt.as_u64();
	// Each index is 9 bits wide, and bits 0..=11 are
	// the offset into the page.
	// As such, `idx1` starts at bit 12, and
	// the others follow at 9-bit offsets.
	let idx4 = addr as usize >> 39 & 0x1FF;
	let idx3 = addr as usize >> 30 & 0x1FF;
	let idx2 = addr as usize >> 21 & 0x1FF;
	let idx1 = addr as usize >> 12 & 0x1FF;
	(idx4, idx3, idx2, idx1)
}

/// A virtual memory allocator.
///
/// A linked list is used to keep track of allocated memory.
/// Each block of allocated memory is preceeded by a [MemoryBlock] node.
/// The [MemoryBlock] is placed as close as possible to the allocated memory
/// while still following it's alignment requirement.
/// This allows the allocator to get an allocated memory's [MemoryBlock]
/// without much trouble.
struct MemoryAllocator {
	first_block: *mut MemoryBlock,
}

impl MemoryAllocator {
	/// Initializes the [MemoryAllocator] and
	/// tells it to use `start_addr` as the start of the heap.
	///
	/// The [MemoryAllocator] sets up an empty [MemoryBlock] at the
	/// start of the heap, which acts as the first node in the linked list.
	/// The first [MemoryBlock] will never get deallocated.
	///
	/// # Safety
	///
	/// - `FRAME_ALLOCATOR.initialize(..)` must have been called
	/// - `MEMORY_MAPPER.initialize(..)` must have been called
	/// - `start_addr` must not point to used memory
	unsafe fn initialize(&mut self, start_addr: u64) {
		// Make sure the page at start_addr is mapped.
		if !MEMORY_MAPPER.is_mapped(VirtAddr::new(start_addr)) {
			let frame = FRAME_ALLOCATOR.allocate_frame();
			MEMORY_MAPPER.map(VirtAddr::new(start_addr), frame);
		}

		// We need to write a MemoryBlock to the start.
		// This MemoryBlock will not keep track of any
		// allocated memory, and is only so that
		// allocating the first time and allocating
		// subsequent times can use the same code
		// (as the first allocation has a previous node).
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
	/// Allocates virtual memory conforming to the given layout.
	///
	/// # Safety
	///
	/// - There must be enough unused space on the heap
	/// - See [`GlobalAlloc::alloc`] for more
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		// Make sure we are not interrupted (lol) while we allocate.
		// We don't want an unexpected interrupt to corrupt the page table!
		x86_64::instructions::interrupts::without_interrupts(|| {
			let mut current_block = self.first_block;

			// Walk through the linked list
			while let Some(next) = (*current_block).next {
				// We need to check if there is enough space
				// between the current block and the next block for
				// the new allocation to fit there.

				// The lowest address the new MemoryBlock can be located
				let block_between_addr = align_up(
					(*current_block).end_of_data_addr(),
					core::mem::align_of::<MemoryBlock>() as _,
				);
				// The lowest address the new allocation can be located
				let data_after_block_addr = align_up(
					block_between_addr + core::mem::size_of::<MemoryBlock>() as u64,
					layout.align() as _,
				);
				// if next.as_ptr() <= data_after_block_addr + layout.size(),
				// then there isn't enough space and we should keep walking the list.
				// Else, we have found a place for our allocation and can stop here.
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

	/// Deallocates virtual memory.
	///
	/// # Safety
	///
	/// See [`GlobalAlloc::dealloc`]
	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		// Same logic as in alloc
		x86_64::instructions::interrupts::without_interrupts(|| {
			// This is the address of the MemoryBlock,
			// as it's placed as close to the allocation as possible.
			let block_addr = align_down(
				ptr as u64 - core::mem::size_of::<MemoryBlock>() as u64,
				core::mem::align_of::<MemoryBlock>() as _,
			);
			let block = (block_addr as *mut MemoryBlock).as_mut().unwrap();

			// There might be pages that are now not used
			// and may be unmapped. However, we must take caution
			// to not unmap any pages which are part of another allocation.
			// If there are any allocations on a page from which
			// we just deallocated, it must be the previous or next
			// allocations.
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
			let max_page_to_unmap = cur_max_page.min(next_min_page - 1);
			for page in min_page_to_unmap..=max_page_to_unmap {
				MEMORY_MAPPER.unmap(VirtAddr::new(page << 12));
			}

			// We need to replace the neighbouring nodes next and prev pointers.
			block.previous.unwrap().as_mut().next = block.next;
			if let Some(mut next) = block.next {
				next.as_mut().previous = block.previous;
			}
		})
	}
}

/// Linked list node keeping track of allocated memory.
struct MemoryBlock {
	previous: Option<NonNull<MemoryBlock>>,
	next: Option<NonNull<MemoryBlock>>,
	/// The layout the memory was allocated with.
	layout: Layout,
	/// Address of the allocated memory.
	data: u64,
}

impl MemoryBlock {
	/// Calculates the address just past the end of the allocated memory.
	fn end_of_data_addr(&self) -> u64 {
		self.data + self.layout.size() as u64
	}

	/// Creates a new allocation just past this one.
	///
	/// # Safety
	///
	/// There must be enough unused space after this [MemoryBlock]'s allocated
	/// memory for a new allocation with the given layout.
	unsafe fn spawn_block(
		&mut self,
		layout: Layout,
		next: Option<NonNull<MemoryBlock>>,
	) -> NonNull<MemoryBlock> {
		// The new MemoryBlock must be past this one's allocated memory,
		// and it must be correctly aligned.
		let block_addr = align_up(self.end_of_data_addr(), core::mem::align_of::<Self>() as _);
		// The data follows, and must also be correctly aligned.
		let data_addr = align_up(
			block_addr + core::mem::size_of::<Self>() as u64,
			layout.align() as _,
		);
		// If the allocated memory's align is higher than that of MemoryBlock,
		// the new block might not be as close at it can be.
		// As such, we recalculate the MemoryBlock's address
		// from the allocated memory's address.
		let block_addr = align_down(
			data_addr - core::mem::size_of::<Self>() as u64,
			core::mem::align_of::<Self>() as _,
		);

		// We also need to make sure that all pages
		// which are spanned by the allocation
		// are mapped.
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
		// Update the previous nodes next pointer,
		// and if there is a node after the new one,
		// update it's previous pointer to point
		// to the new MemoryBlock.
		self.next = Some(ptr);
		if let Some(mut next) = next {
			next.as_mut().previous = Some(ptr);
		}
		ptr
	}
}

/// Initializes all components necessary for dynamic memory allocation.
///
/// Uses `0xFFFFF00000000000` as the start of the heap.
/// Also marks frame 0 as used, to prevent any data being
/// allocated at address 0, which would be indistinguishable from
/// a null (invalid) pointer.
///
/// # Safety
///
/// Must not be called concurrently.
/// Calling this function multiple times might cause undefined behaviour.
pub unsafe fn initialize(mem: &[MemoryRegion]) {
	FRAME_ALLOCATOR.initialize(mem);
	FRAME_ALLOCATOR.set_used(0);
	MEMORY_MAPPER.initialize();
	MEMORY_ALLOCATOR.initialize(0xFFFF_F000_0000_0000);
}

/// The address which physical memory has been mapped to.
const PHYS_MAP_START: u64 = 0xFFFF_FF80_0000_0000;

/// Converts a physical address to a virtual address.
fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
	VirtAddr::new(phys.as_u64() | PHYS_MAP_START)
}

/// Aligns an address down with the givel alignment.
fn align_down(addr: u64, align: u64) -> u64 {
	addr - addr % align
}

/// Aligns an address up with the givel alignment.
fn align_up(addr: u64, align: u64) -> u64 {
	let rest = addr % align;
	if rest == 0 { addr } else { addr + align - rest }
}
