use alloc::{vec, vec::Vec};
use core::{
	borrow::BorrowMut,
	convert::{TryFrom, TryInto},
	hint::unreachable_unchecked,
};

use super::partitions::Partition;
use crate::svec::SVec;

/// The char used for directory seperation (standard is '/', but we are having fun here)
pub const SEPARATOR_CHAR: u8 = b'>';

#[derive(Clone, Debug)]
pub struct FileInfo {
	/// The name of the file (we mostly assume 8.3)
	pub name: SVec<u8, 12>,
	/// Size, in bytes
	pub size: usize,
	/// If the file is, in fact, a directory
	pub is_directory: bool,
	first_cluster: u32,
}

type Path<'a> = &'a [u8];

struct FileAllocationTable {
	version: FatVersion,
	/// The number of FAT sectors
	sector_count: usize,
	/// The offset of were the FAT partion begins on the harddisk
	fat_offset: usize,
	/// Relative to `fat_offset` (harddisk sector)
	currently_loaded_sector: usize,
	/// Buffer two sectors from the harddisk.
	///
	/// We only ever assume one is loaded, but since a cluster could be on a sector boundry, this is to make sure that circumsatance doesn't cause complications.
	buffer: [u8; 1024],
}

impl FileAllocationTable {
	fn new(version: FatVersion, sector_count: usize, fat_offset: usize) -> Self {
		Self {
			version,
			sector_count,
			fat_offset,
			currently_loaded_sector: 0,
			buffer: {
				let mut buffer = [0; 1024];
				unsafe {
					super::partitions::read_sectors(0, fat_offset, &mut buffer);
				}
				buffer
			},
		}
	}

	/// Write the buffer to the disk
	/// Allways run this when changing sectors
	/// (Or use `load_sector_containing`)
	fn flush(&mut self) {
		unsafe {
			super::partitions::write_sectors(
				0,
				self.fat_offset + self.currently_loaded_sector,
				&self.buffer,
			);
		}
	}

	/// Loads the sector containing `cluster`, if there is one.
	fn load_sector_containing(&mut self, cluster: u32) -> Result<(), ()> {
		let bit_offset = cluster as usize * self.version.get_cluster_bit_size();
		let sector_containing_cluster = bit_offset / (512 * 8);

		if sector_containing_cluster >= self.sector_count {
			return Err(());
		}
		if sector_containing_cluster != self.currently_loaded_sector {
			self.flush();
			unsafe {
				super::partitions::read_sectors(
					0,
					self.fat_offset + sector_containing_cluster,
					&mut self.buffer,
				);
			}
			self.currently_loaded_sector = sector_containing_cluster;
		}
		Ok(())
	}

	/// Load the next cluster in the chain, changing sectors as required.
	fn get_next_cluster(&mut self, cluster: u32) -> Option<u32> {
		self.load_sector_containing(cluster).ok()?;

		let bit_offset = cluster as usize * self.version.get_cluster_bit_size();
		let relative_bit_offset = bit_offset - self.currently_loaded_sector * 512 * 8;
		let relative_byte_offset = relative_bit_offset / 8;

		let (next_cluster, invalid_cluster_value) = match self.version {
			FatVersion::Fat12 => {
				// FAT12 stores three nibbles
				let bit_offset_in_byte = relative_bit_offset % 8;
				let num = u16::from_le_bytes([
					self.buffer[relative_byte_offset],
					self.buffer[relative_byte_offset + 1],
				]) >> bit_offset_in_byte;
				let num = num & 0xFFF;
				(num as _, 0xFF8)
			}
			FatVersion::Fat16 => (
				u16::from_le_bytes([
					self.buffer[relative_byte_offset * 2],
					self.buffer[relative_byte_offset * 2 + 1],
				]) as u32,
				0xFFF8,
			),
			FatVersion::Fat32 { .. } => (
				u32::from_le_bytes([
					self.buffer[relative_byte_offset * 4],
					self.buffer[relative_byte_offset * 4 + 1],
					self.buffer[relative_byte_offset * 4 + 2],
					self.buffer[relative_byte_offset * 4 + 3],
				]),
				0x0FFF_FFF8,
			),
		};

		if next_cluster >= invalid_cluster_value {
			None
		} else {
			Some(next_cluster)
		}
	}

	/// Linear search for the next empty cluster.
	fn find_empty_cluster(&mut self, start_cluster: u32) -> Option<u32> {
		for cluster in start_cluster..self.sector_count as u32 * self.clusters_per_sector() as u32 {
			self.load_sector_containing(cluster).ok()?;
			if self.get_next_cluster(cluster) == Some(0) {
				return Some(cluster);
			}
		}
		None
	}

	/// Set the `next_cluster` as being after `cluster` in the chain.
	fn set_next_cluster(&mut self, cluster: u32, next_cluster: Option<u32>) -> Result<(), ()> {
		self.load_sector_containing(cluster)?;

		// let clusters_per_sector = self.clusters_per_sector();
		// let relative_cluster = cluster as usize - self.currently_loaded_sector * clusters_per_sector;

		let bit_offset = cluster as usize * self.version.get_cluster_bit_size();
		let relative_bit_offset = bit_offset - self.currently_loaded_sector * 512 * 8;
		let relative_byte_offset = relative_bit_offset / 8;

		match self.version {
			FatVersion::Fat12 => {
				// FAT12 stores nibbles
				let bit_offset_in_byte = relative_bit_offset % 8;
				let mut num = u16::from_le_bytes([
					self.buffer[relative_byte_offset],
					self.buffer[relative_byte_offset + 1],
				]);

				// 000-xxx_xxx-xxx  --  bit_offset_in_byte = 4
				// xxx-xxx_xxx-000  --  bit_offset_in_byte = 0

				num &= !(0xFFF << bit_offset_in_byte);
				num |= (next_cluster.unwrap_or(0xFFF) as u16) << bit_offset_in_byte;

				self.buffer[relative_byte_offset..relative_byte_offset + 2]
					.copy_from_slice(&num.to_le_bytes());
			}
			FatVersion::Fat16 => {
				self.buffer[relative_byte_offset * 2..(relative_byte_offset + 1) * 2]
					.copy_from_slice(&(next_cluster.unwrap_or(0xFFFF) as u16).to_le_bytes());
			}
			FatVersion::Fat32 { .. } => {
				self.buffer[relative_byte_offset * 4..(relative_byte_offset + 1) * 4]
					.copy_from_slice(&next_cluster.unwrap_or(0x0FFF_FFFF).to_le_bytes());
			}
		};

		Ok(())
	}

	/// Set `cluster` as being empty
	// Is it though?
	fn set_cluster_empty(&mut self, cluster: u32) -> Result<(), ()> {
		self.set_next_cluster(cluster, Some(0))
	}

	/// The number of clusters per (FAT) sector
	fn clusters_per_sector(&self) -> usize {
		let bits_per_sector = 512 * 8;
		let clusters_per_sector = bits_per_sector / self.version.get_cluster_bit_size();
		clusters_per_sector
	}
}

/// The FAT header
#[derive(Debug)]
struct Header {
	oem_ident: SVec<u8, 8>,
	sectors_per_cluster: usize,
	reserved_sectors: usize,
	fat_count: usize,
	root_dir_entries: usize,
	total_sectors: usize,
	sectors_per_fat: usize,
	label: SVec<u8, 11>,

	fat_version: FatVersion,
}

/// The FAT version, also stores the FAT32 unique values
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum FatVersion {
	Fat12,
	Fat16,
	Fat32 {
		root_dir_cluster: u32,
		fsinfo_sector: usize,
	},
}

impl FatVersion {
	/// Returns the number of bits in a cluster for the FAT version.
	// Could this be static/constant? Does it matter?
	fn get_cluster_bit_size(&self) -> usize {
		match self {
			FatVersion::Fat12 => 12,
			FatVersion::Fat16 => 16,
			FatVersion::Fat32 { .. } => 32,
		}
	}
}

impl Header {
	/// Tries to read a a header from the harddisk
	fn try_new(sector: &[u8]) -> Result<Self, ()> {
		let version = if sector[0x16] == 0 {
			let root_dir_cluster =
				u32::from_le_bytes([sector[0x2C], sector[0x2D], sector[0x2E], sector[0x2F]]);
			let fsinfo_sector = u16::from_le_bytes([sector[0x30], sector[0x31]]) as _;
			FatVersion::Fat32 {
				root_dir_cluster,
				fsinfo_sector,
			}
		} else if sector[0x24] == 0x80 {
			FatVersion::Fat16
		} else {
			FatVersion::Fat12
		};

		let oem_ident = {
			let mut ret: SVec<_, 8> = SVec::new();
			for b in &sector[0x03..0x0B] {
				ret.push(*b);
			}
			ret
		};

		if &sector[0x0B..0x0D] != &512u16.to_le_bytes() {
			panic!("Sector size is not 512"); // Shouldn't this be an Error?
		}

		let sectors_per_cluster = sector[0x0D] as _;

		let reserved_sectors = u16::from_le_bytes([sector[0x0E], sector[0x0F]]) as _;

		let fat_count = sector[0x10] as _;

		let root_dir_entries = u16::from_le_bytes([sector[0x11], sector[0x12]]) as _;

		let total_sectors = u16::from_le_bytes([sector[0x13], sector[0x14]]) as _;

		let sectors_per_fat = if let FatVersion::Fat32 { .. } = version {
			u32::from_le_bytes([sector[0x24], sector[0x25], sector[0x26], sector[0x27]])
		} else {
			let count = u16::from_le_bytes([sector[0x16], sector[0x17]]) as u32;
			if count == 0 {
				u32::from_le_bytes([sector[0x20], sector[0x21], sector[0x22], sector[0x23]])
			} else {
				count
			}
		} as _;

		let label = {
			let addr = match version {
				FatVersion::Fat32 { .. } => 0x47,
				_ => 0x2B,
			};

			let mut label = SVec::new();
			for b in &sector[addr..addr + 11] {
				label.push(*b);
			}

			while label[label.len() - 1] == b' ' {
				label.pop();
			}

			label
		};

		Ok(Header {
			oem_ident,
			sectors_per_cluster,
			reserved_sectors,
			fat_count,
			root_dir_entries,
			total_sectors,
			sectors_per_fat,
			label,
			fat_version: version,
		})
	}
}

struct Driver {
	partition: usize,
	header: Header,
	fat: FileAllocationTable,
	/// FAT sector
	current_loaded_sector: usize,
	/// Unlike the `fat`, there is no worry of breaching sector boundries here
	buffer: [u8; 512],
}

static mut DRIVER: Driver = Driver::uninititalized();

impl Driver {
	const fn uninititalized() -> Self {
		Self {
			partition: 0,
			header: Header {
				oem_ident: SVec::new(),
				sectors_per_cluster: 0,
				reserved_sectors: 0,
				fat_count: 0,
				sectors_per_fat: 0,
				label: SVec::new(),
				root_dir_entries: 0,
				fat_version: FatVersion::Fat12,
				total_sectors: 0,
			},
			fat: FileAllocationTable {
				version: FatVersion::Fat12,
				sector_count: 0,
				fat_offset: 0,
				currently_loaded_sector: 0,
				buffer: [0; 1024],
			},
			current_loaded_sector: 0,
			buffer: [0; 512],
		}
	}

	/// Initilize the driver
	///
	/// # Safety
	///
	/// There is no check if the driver has already been initialized
	///
	/// Requires partitions to already be initialized.
	unsafe fn initialize(&mut self) {
		for part in super::partitions::list_partitions() {
			let start = part.start_sector();
			let mut sector = [0; 512];
			super::partitions::read_sectors(part.index(), 0, &mut sector);
			if let Ok(header) = Header::try_new(&sector) {
				self.header = header;
				self.fat = FileAllocationTable::new(
					self.header.fat_version,
					self.header.total_sectors,
					self.header.reserved_sectors,
				);
				super::partitions::read_sectors(part.index(), 0, &mut self.buffer);
				// println!("{:#?}", self.header);
				break;
			}
			// todo!("Check if sector counts are the same for the header and the partition");
		}
	}

	/// Load a particular (FAT) sector
	unsafe fn load_sector(&mut self, sector: usize) {
		if self.current_loaded_sector == sector {
			return;
		}
		self.flush();
		super::partitions::read_sectors(self.partition as _, sector, &mut self.buffer);
		self.current_loaded_sector = sector;
	}

	/// Returns the files/directories inside a cluster
	unsafe fn get_entries_from_cluster(&mut self, cluster: u32) -> Vec<FileInfo> {
		let root_dir_sectors = (self.header.root_dir_entries * 32 + 511) / 512;
		let first_data_sector = self.header.reserved_sectors
			+ self.header.fat_count * self.header.sectors_per_fat
			+ root_dir_sectors;

		let mut file_entries = Vec::new();

		let mut current_cluster = cluster;

		loop {
			let cluster_sector =
				(current_cluster as usize - 2) * self.header.sectors_per_cluster + first_data_sector;
			for sector in cluster_sector..cluster_sector + self.header.sectors_per_cluster {
				self.load_sector(sector);

				for i in 0..(512 / 32) {
					let entry = &self.buffer[i * 32..(i + 1) * 32];
					let entry: DirectoryEntry = entry.try_into().unwrap();

					match entry {
						DirectoryEntry::Standard {
							file_name,
							attributes,
							first_cluster,
							file_size,
						} => {
							// println!("Name: {}", file_name.to_str());
							file_entries.push(FileInfo {
								name: file_name,
								size: file_size as _,
								is_directory: attributes & 0x10 != 0,
								first_cluster,
							});
						}
						DirectoryEntry::LongFileName {} => continue,
						DirectoryEntry::Unused => continue,
						DirectoryEntry::Empty => break,
					}
				}
			}

			if let Some(next_cluster) = self.fat.get_next_cluster(current_cluster) {
				current_cluster = next_cluster;
				continue;
			} else {
				break;
			}
		}

		file_entries
	}

	/// Returns the files/directories of specified path
	///
	/// Empty path gives root directory
	unsafe fn get_entries(&mut self, path: &[u8]) -> Result<Vec<FileInfo>, FatError> {
		unsafe fn get_entries_2(
			s: &mut Driver,
			entries: &[FileInfo],
			path: &[u8],
		) -> Result<Vec<FileInfo>, FatError> {
			let mut parts = path.splitn(2, |v| *v == SEPARATOR_CHAR);
			let first_part = parts.next().unwrap();
			let rest_path = parts.next().unwrap_or(&[]);

			for entry in entries {
				if entry.name.get_slice() == first_part {
					if entry.is_directory {
						let entries = if entry.first_cluster == 0 {
							s.get_root_entries()
						} else {
							s.get_entries_from_cluster(entry.first_cluster)
						};

						if rest_path.len() > 0 {
							return get_entries_2(s, &entries, rest_path);
						} else {
							return Ok(entries);
						}
					} else {
						return Err(FatError::IsntDirectory);
					}
				}
			}

			Err(FatError::PathNotFound)
		}

		if path.len() == 0 {
			Ok(self.get_root_entries())
		} else {
			let root_entries = self.get_root_entries();
			get_entries_2(self, &root_entries, path)
		}
	}

	/// FAT12/16 has special root directories, handled here
	unsafe fn get_root_entries(&mut self) -> Vec<FileInfo> {
		match self.header.fat_version {
			FatVersion::Fat32 {
				root_dir_cluster, ..
			} => self.get_entries_from_cluster(root_dir_cluster),
			_ => {
				let root_dir_sectors = (self.header.root_dir_entries * 32 + 511) / 512;
				let data_sector = self.header.total_sectors
					- (self.header.reserved_sectors
						+ self.header.fat_count * self.header.sectors_per_fat
						+ root_dir_sectors);
				let first_data_sector = self.header.reserved_sectors
					+ self.header.fat_count * self.header.sectors_per_fat
					+ root_dir_sectors;
				let first_root_dir_sector = first_data_sector - root_dir_sectors;

				let mut file_entries = Vec::<FileInfo>::new();

				for i in 0.. {
					let sector = first_root_dir_sector + (i * 32 / 512);
					let index = i % (512 / 32);

					self.load_sector(sector);
					let entry = &self.buffer[index * 32..(index + 1) * 32];
					let entry: DirectoryEntry = entry.try_into().unwrap();

					match entry {
						DirectoryEntry::Standard {
							file_name,
							attributes,
							first_cluster,
							file_size,
						} => {
							file_entries.push(FileInfo {
								name: file_name,
								size: file_size as _,
								is_directory: attributes & 0x10 != 0,
								first_cluster,
							});
						}
						DirectoryEntry::LongFileName {} => continue,
						DirectoryEntry::Unused => continue,
						DirectoryEntry::Empty => break,
					}
				}

				return file_entries;
			}
		}
	}

	/// Loads the data from a file at `path` into `buffer`.
	///
	/// Returns the size of the file succeed or fail.
	unsafe fn read_file(&mut self, path: Path, buffer: &mut [u8]) -> Result<usize, FatError> {
		let file_info = self.get_file_info(path)?;

		if file_info.size > buffer.len() {
			return Err(FatError::BufferTooSmall(file_info.size));
		}

		if file_info.first_cluster == 0 {
			return Ok(0);
		}

		let root_dir_sectors = (self.header.root_dir_entries * 32 + 511) / 512;
		let first_data_sector = self.header.reserved_sectors
			+ self.header.fat_count * self.header.sectors_per_fat
			+ root_dir_sectors;

		let mut current_cluster = file_info.first_cluster;

		let mut cluster_count = 0;
		loop {
			let cluster_sector =
				(current_cluster as usize - 2) * self.header.sectors_per_cluster + first_data_sector;
			for i in 0..self.header.sectors_per_cluster {
				self.load_sector(cluster_sector + i);

				let offset = (cluster_count * self.header.sectors_per_cluster + i) * 512;
				let rest_size = file_info.size.saturating_sub(offset);
				if rest_size > 0 {
					buffer[offset..offset + 512.min(rest_size)]
						.copy_from_slice(&self.buffer[0..rest_size.min(512)]);
				}
			}

			if let Some(next_cluster) = self.fat.get_next_cluster(current_cluster) {
				current_cluster = next_cluster;
			} else {
				break;
			}

			cluster_count += 1;
		}

		Ok(file_info.size)
	}

	/// Returns information about the file at `path`
	unsafe fn get_entry_info(&mut self, path: &[u8]) -> Result<FileInfo, FatError> {
		/*println!(
			"Getting entry info of '{}'",
			core::str::from_utf8(path).unwrap()
		);*/
		if path.len() == 0 {
			return Ok(FileInfo {
				name: SVec::new(),
				size: 0,
				is_directory: true,
				first_cluster: if let FatVersion::Fat32 {
					root_dir_cluster, ..
				} = self.header.fat_version
				{
					root_dir_cluster
				} else {
					0
				},
			});
		}
		assert!(path.len() > 0);

		let mut last_separator_index = None;
		for (i, &b) in path.iter().enumerate() {
			if b == SEPARATOR_CHAR {
				last_separator_index = Some(i);
			}
		}

		let (dir_path, file_name) = if let Some(index) = last_separator_index {
			let parts = path.split_at(index);
			let dir_path = parts.0;
			let file_name = &parts.1[1..];
			(dir_path, file_name)
		} else {
			(&b""[..], path)
		};

		let entries = self.get_entries(dir_path)?;
		for entry in &entries {
			if entry.name.get_slice() == file_name {
				return Ok(entry.clone());
			}
		}

		//println!("Path '{}' not found", core::str::from_utf8(path).unwrap());
		Err(FatError::PathNotFound)
	}

	unsafe fn is_valid_file_path(&mut self, path: &[u8]) -> bool {
		if path.len() == 0 {
			return false;
		}

		let (mut dir_path, mut file_name) = path.split_last_2(&SEPARATOR_CHAR);
		// If user entered a path without separators it will end up in dir_path
		// Assuming user meant a file in root directory so switch them around
		if file_name.len() == 0 {
			core::mem::swap(&mut dir_path, &mut file_name);
		}

		// We only support 8.3 directory entries for now, so need to check the length of file_name and directories
		let (bare_name, extension) = file_name.split_last_2(&b'.');
		if bare_name.len() > 8 || extension.len() > 3 {
			return false;
		}
		let mut start_index: usize = 0;
		for (cur_index, c) in path.iter().enumerate() {
			if *c == SEPARATOR_CHAR {
				if path[start_index..cur_index].len() > 8 {
					return false;
				}
				start_index = cur_index + 1;
			}
		}

		true
	}

	/// Get information about file at `path`
	unsafe fn get_file_info(&mut self, path: &[u8]) -> Result<FileInfo, FatError> {
		let entry = self.get_entry_info(path)?;
		if entry.is_directory {
			Err(FatError::IsDirectory)
		} else {
			Ok(entry)
		}
	}

	/// Get information about directory at `path`
	unsafe fn get_directory_info(&mut self, path: &[u8]) -> Result<FileInfo, FatError> {
		let entry = self.get_entry_info(path)?;
		if !entry.is_directory {
			Err(FatError::IsntDirectory)
		} else {
			Ok(entry)
		}
	}

	/// Creates a empty file at `path`
	///
	/// Assumes 8.3 filename
	///
	/// (aka `touch`)
	unsafe fn create_empty_file(&mut self, path: Path) -> Result<FileInfo, FatError> {
		assert!(self.get_entry_info(path).is_err());

		let (mut dir_path, mut file_name) = path.split_last_2(&SEPARATOR_CHAR);
		if file_name.len() == 0 {
			core::mem::swap(&mut dir_path, &mut file_name);
		}

		if let Err(FatError::PathNotFound) = self.get_directory_info(dir_path) {
			//println!("Creating directory");
			self.create_directory(dir_path)?;
			//println!("Created directory");
		}

		let mut name = SVec::<u8, 8>::new();
		let mut ext = SVec::<u8, 3>::new();

		let (bare_name, extension) = file_name.split_last_2(&b'.');
		for &b in bare_name {
			name.push(b);
		}
		for _ in name.len()..name.capacity() {
			name.push(b' ');
		}
		for &b in extension {
			ext.push(b);
		}
		for _ in ext.len()..ext.capacity() {
			ext.push(b' ');
		}

		for entry_slice in EntryCreatingIterator::new(self, dir_path)? {
			let dir_entry: DirectoryEntry = entry_slice[..].try_into().unwrap();
			match dir_entry {
				DirectoryEntry::Standard { .. } | DirectoryEntry::LongFileName { .. } => continue,
				DirectoryEntry::Unused | DirectoryEntry::Empty => {
					let mut new_entry = [0u8; 32];

					new_entry[0..8].copy_from_slice(name.get_slice());
					new_entry[8..11].copy_from_slice(ext.get_slice());

					entry_slice.copy_from_slice(&new_entry);

					return Ok(FileInfo {
						name: file_name.try_into().unwrap(),
						size: 0,
						is_directory: false,
						first_cluster: 0,
					});
				}
			}
		}

		Err(FatError::FileSystemFull)
	}

	/// Writes a `data` to disk at `path`
	///
	/// # Safety
	///
	/// The dynamic allocator must be initialized.
	unsafe fn write_file(&mut self, path: Path, data: &[u8]) -> Result<(), FatError> {
		// println!("\nStarting to write to file '{}'", core::str::from_utf8(path).unwrap());
		let mut file_info = match self.get_file_info(path) {
			Ok(f) => f,
			Err(FatError::IsDirectory) => return Err(FatError::IsDirectory),
			Err(FatError::PathNotFound) => self.create_empty_file(path)?,
			_ => unreachable!(),
		};

		//println!("File info: {:#?}", file_info);

		let old_size = file_info.size;
		let new_size = data.len();

		let sectors_per_cluster = self.header.sectors_per_cluster;
		let bytes_per_cluster = sectors_per_cluster * 512;
		let old_cluster_count = (old_size + bytes_per_cluster - 1) / bytes_per_cluster;
		let new_cluster_count = (new_size + bytes_per_cluster - 1) / bytes_per_cluster;

		let old_cluster_count = if file_info.first_cluster == 0 {
			let new_cluster = self
				.fat
				.find_empty_cluster(2)
				.ok_or(FatError::FileSystemFull)?;
			self.fat.set_next_cluster(new_cluster, None).unwrap();

			file_info.first_cluster = new_cluster;
			1
		} else {
			old_cluster_count
		};

		if new_cluster_count > old_cluster_count {
			// Extend cluster chain

			let mut current_cluster = file_info.first_cluster;
			while let Some(next_cluster) = self.fat.get_next_cluster(current_cluster) {
				assert_ne!(next_cluster, 0);
				current_cluster = next_cluster;
			}
			let mut last_cluster = current_cluster;

			let clusters_to_allocate = new_cluster_count - old_cluster_count;

			let mut traversed_clusters = vec![];

			for i in 0..clusters_to_allocate {
				let new_cluster = match self.fat.find_empty_cluster(2) {
					Some(new_cluster) => new_cluster,
					None => {
						for cluster in traversed_clusters {
							self.fat.set_cluster_empty(cluster).unwrap();
						}
						self.fat.set_next_cluster(current_cluster, None).unwrap();
						return Err(FatError::FileSystemFull);
					}
				};
				self
					.fat
					.set_next_cluster(last_cluster, Some(new_cluster))
					.unwrap();
				last_cluster = new_cluster;
				traversed_clusters.push(new_cluster);
			}
			self.fat.set_next_cluster(last_cluster, None).unwrap();
		} else if new_cluster_count < old_cluster_count {
			// Truncate cluster chain

			let mut last_cluster = file_info.first_cluster;
			for i in 0..new_cluster_count - 1 {
				last_cluster = self.fat.get_next_cluster(last_cluster).unwrap();
			}
			// last_cluster is now the last cluster in the new chain
			let mut current_cluster = self.fat.get_next_cluster(last_cluster).unwrap();
			self.fat.set_next_cluster(last_cluster, None).unwrap();
			// current_cluster is the first cluster to be removed

			while let Some(next_cluster) = self.fat.get_next_cluster(current_cluster) {
				self.fat.set_cluster_empty(current_cluster).unwrap();
				current_cluster = next_cluster;
			}
			self.fat.set_cluster_empty(current_cluster).unwrap();
		}

		// Write clusters
		let mut current_cluster = file_info.first_cluster;

		let first_data_sector = self.first_data_sector();
		let mut written_cluster_count = 0;

		loop {
			let cluster_start_sector =
				(current_cluster as usize - 2) * sectors_per_cluster + first_data_sector;

			for sector_offset in 0..self.header.sectors_per_cluster {
				self.load_sector(cluster_start_sector + sector_offset);

				let byte_offset = (written_cluster_count * sectors_per_cluster + sector_offset) * 512;
				let rest_size = new_size.saturating_sub(byte_offset).min(512);
				if rest_size > 0 {
					self.buffer[0..rest_size].copy_from_slice(&data[byte_offset..byte_offset + rest_size]);
				}
			}

			match self.fat.get_next_cluster(current_cluster) {
				Some(next_cluster) => current_cluster = next_cluster,
				None => break,
			}
			written_cluster_count += 1;
		}

		file_info.size = new_size;
		self.update_file_info(path, file_info).unwrap();

		self.flush();

		Ok(())
	}

	unsafe fn update_file_info(
		&mut self,
		path: &[u8],
		new_file_info: FileInfo,
	) -> Result<(), FatError> {
		// println!("Updating file info of '{:#?}'", core::str::from_utf8(path).unwrap());
		assert!(self.get_entry_info(path).is_ok());

		let (mut dir_path, mut file_name) = path.split_last_2(&SEPARATOR_CHAR);
		if file_name.len() == 0 {
			core::mem::swap(&mut dir_path, &mut file_name);
		}

		for entry_slice in EntryIterator::new(self, dir_path)? {
			let dir_entry: DirectoryEntry = entry_slice[..].try_into().unwrap();

			match dir_entry {
				DirectoryEntry::Standard {
					file_name: ref name,
					..
				} if name.get_slice() == file_name => {
					let mut dir_entry = dir_entry;
					dir_entry.update(new_file_info);
					let new_file_info: [u8; 32] = dir_entry.into();
					entry_slice.copy_from_slice(&new_file_info);
					return Ok(());
				}
				_ => continue,
			}
		}

		unreachable!()
	}

	unsafe fn create_directory(&mut self, path: Path) -> Result<FileInfo, FatError> {
		let mut existing_path = &b""[..];
		let mut rest_path = path;

		// "dir>temp"
		//   "", "dir"
		//   "dir", "temp"
		//

		let mut parent_dir_cluster = match self.header.fat_version {
			FatVersion::Fat32 {
				root_dir_cluster,
				fsinfo_sector,
			} => root_dir_cluster,
			_ => 0,
		};

		let mut latest_file_info = FileInfo {
			name: SVec::new(),
			size: 0,
			is_directory: true,
			first_cluster: parent_dir_cluster,
		};

		let mut separator_indices = path
			.iter()
			.enumerate()
			.filter_map(|(i, c)| if *c == SEPARATOR_CHAR { Some(i) } else { None })
			.collect::<Vec<_>>();
		separator_indices.insert(0, 0);
		separator_indices.push(path.len());
		'path_parts_loop: for i in 0..separator_indices.len() - 1 {
			let parent = &path[..separator_indices[i]];
			let dir_to_create = &path[separator_indices[i] + 1.min(i)..separator_indices[i + 1]];
			/*println!(
				"Creating dir {} in {}",
				core::str::from_utf8(dir_to_create).unwrap(),
				core::str::from_utf8(parent).unwrap()
			);*/

			for entry_slice in EntryIterator::new(self, parent)? {
				let dir_entry: DirectoryEntry = entry_slice[..].try_into().unwrap();

				match dir_entry {
					DirectoryEntry::Standard {
						file_name,
						first_cluster,
						..
					} if file_name.get_slice() == dir_to_create => {
						/*println!(
							"Found existing dir {}",
							core::str::from_utf8(dir_to_create).unwrap()
						);*/
						parent_dir_cluster = first_cluster;
						latest_file_info = FileInfo {
							name: file_name,
							size: 0,
							is_directory: true,
							first_cluster,
						};
						continue 'path_parts_loop;
					}
					_ => continue,
				}
			}

			for entry_slice in EntryCreatingIterator::new(self, parent)? {
				let dir_entry: DirectoryEntry = entry_slice[..].try_into().unwrap();

				match dir_entry {
					DirectoryEntry::Empty | DirectoryEntry::Unused => {
						/*println!(
							"Creating dir {}",
							core::str::from_utf8(dir_to_create).unwrap()
						);*/
						let entry_slice_ptr = entry_slice.as_mut_ptr();
						let old_sector = self.current_loaded_sector;

						let dir_cluster = self
							.fat
							.find_empty_cluster(2)
							.ok_or(FatError::FileSystemFull)?;
						self.fat.set_next_cluster(dir_cluster, None).unwrap();
						let cluster_sector = (dir_cluster as usize - 2) * self.header.sectors_per_cluster
							+ self.first_data_sector();

						for sector_offset in 0..self.header.sectors_per_cluster {
							self.load_sector(cluster_sector + sector_offset);
							self.buffer = core::mem::zeroed();
						}

						self.load_sector(cluster_sector);

						let current_directory_entry = DirectoryEntry::Standard {
							file_name: {
								let mut name = SVec::new();
								name.push(b'.');
								name.push(b'.'); // lol lmao
								name
							},
							attributes: 0x10, // directory
							first_cluster: dir_cluster,
							file_size: 0,
						};

						let parent_directory_entry = DirectoryEntry::Standard {
							file_name: {
								let mut name = SVec::new();
								name.push(b'.');
								name.push(b'.');
								name.push(b'.');
								name
							},
							attributes: 0x10, // Directory
							first_cluster: parent_dir_cluster,
							file_size: 0,
						};

						let temp: [u8; 32] = current_directory_entry.into();
						self.buffer[0..32].copy_from_slice(&temp[..]);

						let temp: [u8; 32] = parent_directory_entry.into();
						self.buffer[32..64].copy_from_slice(&temp[..]);

						//println!("buffer: {:x?}", self.buffer);

						self.load_sector(old_sector);

						let dir_name = {
							let mut name = SVec::new();
							for b in dir_to_create {
								name.push(*b);
							}
							name
						};

						let dir_to_create_entry = DirectoryEntry::Standard {
							file_name: dir_name.clone(),
							attributes: 0x10, // directory
							first_cluster: dir_cluster,
							file_size: 0,
						};

						let entry_slice = core::slice::from_raw_parts_mut(entry_slice_ptr, 32);

						let temp: [u8; 32] = dir_to_create_entry.into();
						entry_slice.copy_from_slice(&temp[..]);

						parent_dir_cluster = dir_cluster;

						latest_file_info = FileInfo {
							name: dir_name,
							size: 0,
							is_directory: true,
							first_cluster: dir_cluster,
						};

						continue 'path_parts_loop;
					}
					_ => continue,
				}
			}
		}

		Ok(latest_file_info)
	}

	fn first_data_sector(&mut self) -> usize {
		let root_dir_sectors = (self.header.root_dir_entries * 32 + 511) / 512;
		let first_data_sector = self.header.reserved_sectors
			+ self.header.fat_count * self.header.sectors_per_fat
			+ root_dir_sectors;
		first_data_sector
	}

	/// Writes the buffer to disk
	fn flush(&mut self) {
		unsafe {
			super::partitions::write_sectors(0, self.current_loaded_sector, &self.buffer);
		}
	}
}

/// USING THIS MAY CAUSE UNDEFINED BEHAVIOUR
/// USE AT YOUR OWN RISK
struct EntryCreatingIterator<'a> {
	inner: &'a mut Driver,
	is_root_directory: bool,
	/// If [`Self::is_root_directory`] is true, this is absolute.
	/// Else, this is relative to the current cluster.
	next_entry_index: usize,
	next_cluster: Option<u32>,
}

impl<'a> EntryCreatingIterator<'a> {
	unsafe fn new(driver: &'a mut Driver, path: Path) -> Result<Self, FatError> {
		let dir_info = driver.get_directory_info(path)?;

		let is_root_directory = dir_info.first_cluster == 0;

		Ok(Self {
			inner: driver,
			is_root_directory,
			next_entry_index: 0,
			next_cluster: if is_root_directory {
				None
			} else {
				Some(dir_info.first_cluster)
			},
		})
	}
}

impl<'a> Iterator for EntryCreatingIterator<'a> {
	type Item = &'a mut [u8; 32];

	fn next(&mut self) -> Option<Self::Item> {
		unsafe {
			if self.is_root_directory {
				// Root directory

				if self.next_entry_index >= self.inner.header.root_dir_entries {
					return None;
				}

				let root_dir_sectors = (self.inner.header.root_dir_entries * 32 + 511) / 512;
				let first_root_dir_sector = self.inner.header.reserved_sectors
					+ self.inner.header.fat_count * self.inner.header.sectors_per_fat;

				let sector_offset = self.next_entry_index / (512 / 32);

				self
					.inner
					.load_sector(first_root_dir_sector + sector_offset);

				let entry_offset = self.next_entry_index % 32;

				let entry = &mut self.inner.buffer[entry_offset * 32..(entry_offset + 1) * 32];
				let dir_entry: DirectoryEntry = entry.try_into().unwrap();
				match dir_entry {
					_ => {
						self.next_entry_index += 1;
						return Some(entry.as_mut_ptr().cast::<[u8; 32]>().as_mut().unwrap());
					}
				}
			} else if let Some(current_cluster) = self.next_cluster {
				let root_dir_sectors = (self.inner.header.root_dir_entries * 32 + 511) / 512;
				let first_data_sector = self.inner.header.reserved_sectors
					+ self.inner.header.fat_count * self.inner.header.sectors_per_fat
					+ root_dir_sectors;
				let first_root_dir_sector = first_data_sector - root_dir_sectors;

				let mut current_cluster = current_cluster;

				// println!("current cluster: {}", current_cluster);
				let cluster_sector = (current_cluster as usize - 2) * self.inner.header.sectors_per_cluster
					+ first_data_sector;

				let sector_offset = self.next_entry_index / (512 / 32);
				let sector = cluster_sector + sector_offset;

				// println!("Starting sector {}", sector);
				self.inner.load_sector(sector);

				let entry_offset = self.next_entry_index % (512 / 32);

				let entry = &mut self.inner.buffer[entry_offset * 32..(entry_offset + 1) * 32];
				let dir_entry: DirectoryEntry = entry.try_into().unwrap();

				match dir_entry {
					_ => {
						self.next_entry_index += 1;

						if self.next_entry_index >= self.inner.header.sectors_per_cluster * (512 / 32) {
							self.next_entry_index = 0;

							self.next_cluster = match self.inner.fat.get_next_cluster(current_cluster) {
								cluster @ Some(_) => cluster,
								None => {
									//println!("Allocating new cluster");
									let new_cluster = self.inner.fat.find_empty_cluster(2)?;
									for sector_offset in 0..self.inner.header.sectors_per_cluster {
										let cluster_sector = (new_cluster as usize - 2)
											* self.inner.header.sectors_per_cluster
											+ first_data_sector;

										self.inner.load_sector(cluster_sector + sector_offset);
										self.inner.buffer = core::mem::zeroed();
									}
									self
										.inner
										.fat
										.set_next_cluster(self.next_cluster.unwrap(), Some(new_cluster))
										.ok()?;
									self.inner.fat.set_next_cluster(new_cluster, None).ok()?;
									Some(new_cluster)
								}
							};

							self.inner.load_sector(sector);
							let entry = &mut self.inner.buffer[entry_offset * 32..(entry_offset + 1) * 32];
							return Some(entry.as_mut_ptr().cast::<[u8; 32]>().as_mut().unwrap());
						} else {
							return Some(entry.as_mut_ptr().cast::<[u8; 32]>().as_mut().unwrap());
						}
					}
				}
			} else {
				return None;
			}
		}
	}
}

/// USING THIS MAY CAUSE UNDEFINED BEHAVIOUR
/// USE AT YOUR OWN RISK
struct EntryIterator<'a> {
	inner: &'a mut Driver,
	is_root_directory: bool,
	/// If [`Self::is_root_directory`] is true, this is absolute.
	/// Else, this is relative to the current cluster.
	next_entry_index: usize,
	next_cluster: Option<u32>,
}

impl<'a> EntryIterator<'a> {
	unsafe fn new(driver: &'a mut Driver, path: Path) -> Result<Self, FatError> {
		let dir_info = driver.get_directory_info(path)?;

		let is_root_directory = dir_info.first_cluster == 0;

		Ok(Self {
			inner: driver,
			is_root_directory,
			next_entry_index: 0,
			next_cluster: if is_root_directory {
				None
			} else {
				Some(dir_info.first_cluster)
			},
		})
	}
}

impl<'a> Iterator for EntryIterator<'a> {
	type Item = &'a mut [u8; 32];

	fn next(&mut self) -> Option<Self::Item> {
		unsafe {
			if self.is_root_directory {
				// Root directory

				if self.next_entry_index >= self.inner.header.root_dir_entries {
					return None;
				}

				let root_dir_sectors = (self.inner.header.root_dir_entries * 32 + 511) / 512;
				let first_root_dir_sector = self.inner.header.reserved_sectors
					+ self.inner.header.fat_count * self.inner.header.sectors_per_fat;

				let sector_offset = self.next_entry_index / (512 / 32);

				self
					.inner
					.load_sector(first_root_dir_sector + sector_offset);

				let entry_offset = self.next_entry_index % 32;

				let entry = &mut self.inner.buffer[entry_offset * 32..(entry_offset + 1) * 32];
				let dir_entry: DirectoryEntry = entry.try_into().unwrap();
				match dir_entry {
					DirectoryEntry::Empty => return None,
					_ => {
						self.next_entry_index += 1;
						return Some(entry.as_mut_ptr().cast::<[u8; 32]>().as_mut().unwrap());
					}
				}
			} else if let Some(current_cluster) = self.next_cluster {
				let root_dir_sectors = (self.inner.header.root_dir_entries * 32 + 511) / 512;
				let first_data_sector = self.inner.header.reserved_sectors
					+ self.inner.header.fat_count * self.inner.header.sectors_per_fat
					+ root_dir_sectors;
				let first_root_dir_sector = first_data_sector - root_dir_sectors;

				let mut current_cluster = current_cluster;

				// println!("current cluster: {}", current_cluster);
				let cluster_sector = (current_cluster as usize - 2) * self.inner.header.sectors_per_cluster
					+ first_data_sector;

				let sector_offset = self.next_entry_index / (512 / 32);
				let sector = cluster_sector + sector_offset;

				// println!("Starting sector {}", sector);
				self.inner.load_sector(sector);

				let entry_offset = self.next_entry_index % (512 / 32);

				let entry = &mut self.inner.buffer[entry_offset * 32..(entry_offset + 1) * 32];
				let dir_entry: DirectoryEntry = entry.try_into().unwrap();

				match dir_entry {
					DirectoryEntry::Empty => return None,
					_ => {
						self.next_entry_index += 1;

						if self.next_entry_index >= self.inner.header.sectors_per_cluster * (512 / 32) {
							self.next_entry_index = 0;

							self.next_cluster = self.inner.fat.get_next_cluster(current_cluster);
						}

						return Some(entry.as_mut_ptr().cast::<[u8; 32]>().as_mut().unwrap());
					}
				}
			} else {
				return None;
			}
		}
	}
}

#[derive(Debug)]
pub enum FatError {
	PathNotFound,
	IsntDirectory,
	IsDirectory,
	/// How big the file is
	BufferTooSmall(usize),
	FileSystemFull,
}

enum DirectoryEntry {
	Standard {
		file_name: SVec<u8, 12>,
		attributes: u8,
		first_cluster: u32,
		file_size: u32,
	},
	LongFileName {},
	Unused,
	Empty,
}

impl DirectoryEntry {
	fn update(&mut self, file_info: FileInfo) {
		match self {
			DirectoryEntry::Standard {
				file_name,
				attributes,
				first_cluster,
				file_size,
			} => {
				*file_name = file_info.name;
				// Set or clear directory flag (0x10) depending on file_info.is_directory
				*attributes = if file_info.is_directory { 0x10 } else { 0x00 } | (*attributes & !0x10);
				*first_cluster = file_info.first_cluster;
				*file_size = file_info.size as _;
			}
			_ => unimplemented!(),
		}
	}
}

impl TryFrom<&[u8]> for DirectoryEntry {
	type Error = ();

	/// Tries to get information about the directory
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		if value.len() != 32 {
			return Err(());
		}
		match value[0] {
			0x00 => return Ok(DirectoryEntry::Empty),
			0xE5 => return Ok(DirectoryEntry::Unused),
			_ => {}
		}

		let attributes = value[11];
		if attributes == 0x0F {
			return Ok(Self::LongFileName {});
		}

		let mut bare_name: SVec<_, 8> = SVec::new();
		let mut ext: SVec<_, 3> = SVec::new();

		for b in &value[..8] {
			bare_name.push(*b);
		}
		while bare_name.len() > 0 && bare_name[bare_name.len() - 1] == b' ' {
			bare_name.pop();
		}

		for b in &value[8..11] {
			ext.push(*b);
		}
		while ext.len() > 0 && ext[ext.len() - 1] == b' ' {
			ext.pop();
		}

		let mut filename = SVec::new();

		for b in bare_name.get_slice() {
			filename.push(*b);
		}
		if ext.len() > 0 {
			filename.push(b'.');
			for b in ext.get_slice() {
				filename.push(*b);
			}
		}

		let cluster_high = u16::from_le_bytes([value[20], value[21]]) as u32;

		let cluster_low = u16::from_le_bytes([value[26], value[27]]) as u32;

		let cluster = cluster_high << 16 | cluster_low;

		let file_size = u32::from_le_bytes([value[28], value[29], value[30], value[31]]);

		Ok(DirectoryEntry::Standard {
			file_name: filename,
			attributes,
			first_cluster: cluster,
			file_size,
		})
	}
}

impl TryFrom<&mut [u8]> for DirectoryEntry {
	type Error = ();

	fn try_from(value: &mut [u8]) -> Result<Self, Self::Error> {
		(&*value).try_into()
	}
}

impl Into<[u8; 32]> for DirectoryEntry {
	fn into(self) -> [u8; 32] {
		match self {
			DirectoryEntry::Standard {
				file_name,
				attributes,
				first_cluster,
				file_size,
			} => {
				let mut ret = [0; 32];

				let mut name = SVec::<u8, 8>::new();
				let mut ext = SVec::<u8, 3>::new();
				let (bare_name, extension) = file_name.get_slice().split_last_2(&b'.');
				for &b in bare_name {
					name.push(b);
				}
				for _ in name.len()..name.capacity() {
					name.push(b' ');
				}
				for &b in extension {
					ext.push(b);
				}
				for _ in ext.len()..ext.capacity() {
					ext.push(b' ');
				}

				ret[0..8].copy_from_slice(name.get_slice());
				ret[8..11].copy_from_slice(ext.get_slice());
				ret[11] = attributes;
				ret[20..22].copy_from_slice(&((first_cluster >> 16) as u16).to_le_bytes());
				ret[26..28].copy_from_slice(&(first_cluster as u16).to_le_bytes());
				ret[28..32].copy_from_slice(&file_size.to_le_bytes());

				ret
			}
			_ => unimplemented!(),
		}
	}
}

/// Initializes the FAT32 driver
///
/// # Safety
///
/// Requires partitions to have been initialized
pub unsafe fn initialize() {
	DRIVER.initialize();
	// for file in driver.get_entries(b"").unwrap().get_slice() {
	// 	println!(
	// 		"{:12}  {:3}  {}",
	// 		file.name.to_str(),
	// 		if file.is_directory { "DIR" } else { "   " },
	// 		file.size
	// 	);
	// }
}

/// Writes `data` to `path`
pub unsafe fn write_file(path: Path, data: &[u8]) -> Result<(), FatError> {
	DRIVER.write_file(path, data)
}

/// Puts the data from `path` in `buffer`
///
/// Returns size of file, succeed or fail.
pub unsafe fn read_file(path: Path, buffer: &mut [u8]) -> Result<usize, FatError> {
	DRIVER.read_file(path, buffer)
}

/// Get the `FileInfo` for the file at `path`
pub unsafe fn get_file_info(path: Path) -> Result<FileInfo, FatError> {
	DRIVER.get_entry_info(path)
}

/// Returns ok if path contains a valid file name and the directory path exists
pub unsafe fn is_valid_file_path(path: Path) -> bool {
	DRIVER.is_valid_file_path(path)
}

/// Lists all entries in `directory_path`
pub unsafe fn list_entries(directory_path: Path) -> Result<Vec<FileInfo>, FatError> {
	DRIVER.get_entries(directory_path)
}

/// `touch`
///
/// Creates an empty file at `path`
pub unsafe fn create_empty_file(path: Path) -> Result<FileInfo, FatError> {
	DRIVER.create_empty_file(path)
}

/// Used to split directories from each other in paths
pub trait SplitLast<T>: Sized {
	fn split_last_2(self, v: &T) -> (Self, Self);
}

impl<T: PartialEq> SplitLast<T> for &[T] {
	/// Splits at the last occurence of `v`
	///
	/// # Example
	/// ```
	/// let s:SVec<char, 6> = SVec::new();
	/// s.push("r")
	/// s.push("o")
	/// s.push("o")
	/// s.push("t")
	/// s.push("/")
	/// s.push("d")
	/// s.push("i")
	/// s.push("r")
	/// s.push("/")
	/// s.push("f")
	/// s.push("i")
	/// s.push("l")
	/// s.push("e")
	///
	/// asserteq!(("root".as_slice(), "dir/file".as_slice()), s.get_slice().split_last_2("/"));
	fn split_last_2(self, v: &T) -> (Self, Self) {
		let mut last_separator_index = None;
		for (i, b) in self.iter().enumerate() {
			if b == v {
				last_separator_index = Some(i);
			}
		}

		if let Some(index) = last_separator_index {
			let parts = self.split_at(index);
			let dir_path = parts.0;
			let file_name = &parts.1[1..];
			(dir_path, file_name)
		} else {
			(self, &[][..])
		}
	}
}
