use crate::{harddisk::pata, svec::SVec};

// Layouts from OSDev wiki: https://wiki.osdev.org/GPT
//
// GPT partitioned media layout:
// LBA0: Protective Master Boot Record. Kept for backward comptibility.
// LBA1: Partition header, identified by 8 bytes "EFI PART", [0x45 0x46 0x49 0x20 0x50 0x41 0x52 0x54]
// LBA2..33: Partition table entries
// LBA-2: Mirror of partition table
// LBA-1: Mirror of partition header
//
// Partition Table Header layout:
// 0x00 (8) - Signature "EFI PART", [0x45 0x46 0x49 0x20 0x50 0x41 0x52 0x54]
// 0x08 (4) - GPT Revision
// 0x0C (4) - Header size
// 0x10 (4) - CRC32 Checksum
// 0x14 (4) - Reserved
// 0x18 (8) - The LBA containing this header
// 0x20 (8) - The LBA of the alternative GPT header
// 0x28 (8) - First usable LBA
// 0x30 (8) - Last usable LBA
// 0x38 (16) - Disk GUID
// 0x48 (8) - Starting LBA of Partition Entry array
// 0x50 (4) - Number of Partition Entries
// 0x54 (4) - Size of a Partition Entry (multiple of 8, usually 0x80 or 0x128)
// 0x58 (4) - CRC32 Checksum of Partition Entry array
// 0x5C (*) - Reserved, must be zeroed (420 bytes for a 512 byte sector size)
//
// Partition Entires layout
// 0x00 (16) - Partition Type GUID
// 0x10 (16) - Unique partition GUID
// 0x20 (8) - First LBA
// 0x28 (8) - Last LBA
// 0x30 (8) - Attribute flags
// 0x38 (72) - Partition name

const NUM_PARTITIONS: usize = 16;

static mut PARTITIONS: SVec<Partition, NUM_PARTITIONS> = SVec::new();

pub struct Partition {
	index: u8,
	partition_guid: [u8; 16],
	start_sector: usize,
	sector_count: usize,
	name: SVec<char, 36>,
}

impl Partition {
	pub fn index(&self) -> u8 {
		self.index
	}

	pub fn partition_guid(&self) -> &[u8] {
		&self.partition_guid
	}

	pub fn start_sector(&self) -> usize {
		self.sector_count
	}

	pub fn sector_count(&self) -> usize {
		self.sector_count
	}

	pub fn name(&self) -> &SVec<char, 36> {
		&self.name
	}
}

/// Initializes and populates the partition information array for disk 0
///
/// # Safety
///
/// Assumes sector size is 512 bytes
///
/// The module 'pata' must be initialized before this function is called
pub unsafe fn initialize() {
	let mut buf = [0 as u8; 512];

	// Read GPT Header from disk (sector 1)
	pata::read_sectors(0, 1, &mut buf);
	// Make sure it's a GPT header
	if !buf.starts_with(&[0x45, 0x46, 0x49, 0x20, 0x50, 0x41, 0x52, 0x54]) {
		panic!("No GUID Partition Table found on disk");
	}

	// Assuming the data exists and that it's correct for now
	// Might want to compare checksums etc

	// Start sector for partition entries
	let start_sector = usize::from_le_bytes([
		buf[0x48], buf[0x49], buf[0x4A], buf[0x4B], buf[0x4C], buf[0x4D], buf[0x4E], buf[0x4F],
	]);
	// Number of partition entries. Currently not used.. hard coded to max 16 partitions for now
	//let num_partition_entries = u32::from_le_bytes([buf[0x50], buf[0x51], buf[0x52], buf[0x53]]);
	// Size of partition entry
	let partition_entry_size = u32::from_le_bytes([buf[0x54], buf[0x55], buf[0x56], buf[0x57]]);

	//println!("Start sector for partition entries: {}", start_sector);
	//println!("Number of partition entries: {}", num_partition_entries);
	//println!("Size of a partition entry: {}", partition_entry_size);
	//print!("\n");

	// Read partition entries (only the first 16 for now)
	let num_entries_per_slice = 512 / partition_entry_size;
	let last_sector = start_sector + (NUM_PARTITIONS / num_entries_per_slice as usize);
	let mut partition_index: u8 = 0;
	for s in start_sector..last_sector {
		// Read disk sector
		pata::read_sectors(0, s, &mut buf);
		// Read individual partition entry
		for p in 0..num_entries_per_slice {
			let base_offset: usize = (partition_entry_size * p) as usize;
			let mut offset = base_offset;

			// Read partition type
			let mut partition_type_guid = [0 as u8; 16];
			partition_type_guid.copy_from_slice(&buf[offset..offset + 0x10]);
			// Skip unused entries
			if partition_type_guid == [0; 16] {
				continue;
			}

			// Read partition guid
			offset = base_offset + 0x10;
			let mut partition_guid = [0 as u8; 16];
			partition_guid.copy_from_slice(&buf[offset..offset + 0x10]);

			// Read first and last sector
			offset = base_offset + 0x20;
			let start_sector = usize::from_le_bytes([
				buf[offset],
				buf[offset + 1],
				buf[offset + 2],
				buf[offset + 3],
				buf[offset + 4],
				buf[offset + 5],
				buf[offset + 6],
				buf[offset + 7],
			]);
			offset = base_offset + 0x28;
			let last_sector = usize::from_le_bytes([
				buf[offset],
				buf[offset + 1],
				buf[offset + 2],
				buf[offset + 3],
				buf[offset + 4],
				buf[offset + 5],
				buf[offset + 6],
				buf[offset + 7],
			]);

			// Read name
			let mut name: SVec<char, 36> = SVec::new();
			offset = base_offset + 0x38;
			// EFI spec says 72 bytes (36 characters), however OSDev wiki says never to hardcode this and use {partition_entry_size - offset} instead.
			// Since we don't support dynamic allocation, use the shortest length.
			let name_length = core::cmp::min(36, partition_entry_size - 0x38);
			for _n in 0..name_length {
				let c = u16::from_le_bytes([buf[offset], buf[offset + 1]]);
				if c == 0x0000 {
					break;
				}
				offset += 2;
				name.push(char::from_u32(c as u32).unwrap());
			}

			// Make partition entry
			let entry = Partition {
				index: partition_index,
				partition_guid,
				start_sector,
				sector_count: (last_sector - start_sector),
				name,
			};

			//println!("drive: {}", drive);
			//println!("partition guid: {:X?}", partition_guid);
			//println!("start sector: {}", start_sector);
			//println!("sector count: {}", last_sector-start_sector);
			//print!("Name: ");
			//for i in 0..entry.name.len() {
			//    print!("{}", entry.name[i]);
			//}
			//print!("\n");

			// Push to partition list
			PARTITIONS.push(entry);

			// Increase drive index
			partition_index += 1;
		}
	}
}

pub unsafe fn list_partitions() -> &'static [Partition] {
	return PARTITIONS.get_slice();
}

/// Reads sectors from specified partition
/// start_sector starts at 0
pub unsafe fn read_sectors(partition: u8, start_sector: usize, buffer: &mut [u8]) {
	if buffer.len() % 512 != 0 {
		panic!("Buffer must be a multiple of 512 bytes");
	}

	let sector_count = PARTITIONS[partition as usize].sector_count;
	if start_sector >= sector_count {
		panic!("sector out of range");
	}

	let sector = PARTITIONS[partition as usize].start_sector + start_sector;
	pata::read_sectors(partition, sector, buffer);
}

// Writes sectors to specified partition
/// start_sector starts at 0
pub unsafe fn write_sectors(partition: u8, start_sector: usize, buffer: &[u8]) {
	if buffer.len() % 512 != 0 {
		panic!("Buffer must be a multiple of 512 bytes");
	}

	if buffer.len() % 512 != 0 {
		panic!("Buffer must be a multiple of 512 bytes");
	}

	let sector_count = PARTITIONS[partition as usize].sector_count;
	if start_sector >= sector_count {
		panic!("sector out of range");
	}

	let sector = PARTITIONS[partition as usize].start_sector + start_sector;
	pata::write_sectors(partition, sector, buffer);
}
