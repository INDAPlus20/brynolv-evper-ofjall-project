use core::{sync::atomic::AtomicBool, usize};

use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

use crate::svec::SVec;

// Assuming "typical" ports
const IO_BASE_PORT: u16 = 0x1F0;
/// 0x1F0
static mut DATA_REG: Port<u16> = Port::new(IO_BASE_PORT);
/// 0x1F1
static mut ERROR_REG: PortReadOnly<u16> = PortReadOnly::new(IO_BASE_PORT + 1);
/// 0x1F1
static mut FEATURES_REG: PortWriteOnly<u16> = PortWriteOnly::new(IO_BASE_PORT + 1);
/// 0x1F2
static mut SECTOR_COUNT_REG: Port<u8> = Port::new(IO_BASE_PORT + 2); //Actually u16, but low and high needs to be sent separately.
/// 0x1F3
static mut LBA_LOW_REG: Port<u8> = Port::new(IO_BASE_PORT + 3); //same
/// 0x1F4
static mut LBA_MID_REG: Port<u8> = Port::new(IO_BASE_PORT + 4); // with these 2
/// 0x1F5
static mut LBA_HIGH_REG: Port<u8> = Port::new(IO_BASE_PORT + 5);
/// 0x1F6
static mut DRIVE_HEAD_REG: Port<u8> = Port::new(IO_BASE_PORT + 6);
/// 0x1F7
static mut STATUS_REG: PortReadOnly<u8> = PortReadOnly::new(IO_BASE_PORT + 7);
/// 0x1F7
static mut COMMAND_REG: Port<u8> = Port::new(IO_BASE_PORT + 7);
const CONTROL_BASE_PORT: u16 = 0x3F6;
/// 0x3F6
static mut ALT_STATUS_REG: PortReadOnly<u8> = PortReadOnly::new(CONTROL_BASE_PORT);
/// 0x3F6
static mut DEVICE_CONTROL_REG: PortWriteOnly<u8> = PortWriteOnly::new(CONTROL_BASE_PORT + 0);
/// 0x3F7
static mut DRIVE_ADRESS_REG: PortReadOnly<u8> = PortReadOnly::new(CONTROL_BASE_PORT + 1);

/// Is the driver busy?
/// Since only one port is used, the two drives on it will have to go one at a time.
/// TODO: make this per disk
static BUSY: AtomicBool = AtomicBool::new(false);
/// What is the maximum `iter` acheived during `poll()`?
/// Used to compensate for fast/slow CPUs
static mut MAX_ITER: usize = 1000;

/// Contains the information on the drives/disks
static mut DRIVES: SVec<DriveInfo, 2> = SVec::new();

/// Intitialize the primary drive bus, and all drives on it.
/// # Safety
/// All port I/O can threaten safety.
///
/// `printer` should be initialized for panic messages.
pub unsafe fn initialize() {
	let status = STATUS_REG.read();
	if status == 0xFF {
		panic!("Floating bus");
	}
	DEVICE_CONTROL_REG.write(0);
	for drive in 0..DRIVES.capacity() {
		DRIVES.push(initialize_drive(drive as u8));
	}
}

/// Initializes a particular drive, and returns it's info.
unsafe fn initialize_drive(drive: u8) -> DriveInfo {
	let mut disk = DriveInfo {
		drive,
		status: DriveStatus::Unknown,
		sectors: 0,
		lba48: false,
		identify_result: [0; 256],
	};
	DRIVE_HEAD_REG.write(0xA0 + (drive << 4));
	wait_till_idle();
	send_lba_and_sector_count(0, 0, false);
	COMMAND_REG.write(0xEC); //IDENTIFY
	let status = STATUS_REG.read();
	if status == 0 {
		disk.status = DriveStatus::Disconnected;
		return disk;
	}
	wait_till_idle();
	if LBA_MID_REG.read() != 0 || LBA_HIGH_REG.read() != 0 {
		disk.status = DriveStatus::Unknown;
		return disk;
	}
	loop {
		let status = STATUS_REG.read();
		if status & 8 == 8 {
			break;
		}
		if status & 1 == 1 {
			disk.status = DriveStatus::Unknown;
			return disk;
		}
	}
	for i in 0..256 {
		disk.identify_result[i] = DATA_REG.read();
	}
	// bit 10
	if disk.identify_result[83] & 0x200 != 0x200 {
		let mut bytes: [u8; 4] = [0; 4];
		let low = disk.identify_result[60].to_le_bytes();
		bytes[0] = low[0];
		bytes[1] = low[1];
		let high = disk.identify_result[61].to_le_bytes();
		bytes[2] = high[0];
		bytes[3] = high[1];
		let lba28 = u32::from_le_bytes(bytes);
		if lba28 != 0 {
			disk.sectors = lba28 as usize;
		}
	} else {
		let mut bytes: [u8; 8] = [0; 8];
		let b100 = disk.identify_result[100].to_le_bytes();
		for i in 0..b100.len() {
			bytes[i] = b100[i];
		}
		let b101 = disk.identify_result[101].to_le_bytes();
		for i in 0..b101.len() {
			bytes[i + 2] = b101[i];
		}
		let b102 = disk.identify_result[102].to_le_bytes();
		for i in 0..b102.len() {
			bytes[4 + i] = b102[i];
		}
		let b103 = disk.identify_result[103].to_le_bytes();
		for i in 0..b103.len() {
			bytes[6 + i] = b103[i];
		}
		disk.lba48 = true;
		disk.sectors = u64::from_le_bytes(bytes) as usize;
	}
	disk.status = DriveStatus::Connected;
	wait_till_idle();
	disk
}

/// Info about the particular drive
#[derive(Clone)]
pub struct DriveInfo {
	/// Drive number sent to R/W function
	pub drive: u8,
	/// The status of the drive (See enum for details)
	pub status: DriveStatus,
	/// Sectors available on drive
	/// Should only be used by partition driver.
	pub sectors: usize,
	/// Does the drive support lba48?
	lba48: bool,
	/// The result of original IDENTIFY command
	identify_result: [u16; 256],
}

/// The status of the drive at initialization.
#[derive(Clone, PartialEq, Eq)]
pub enum DriveStatus {
	/// Drive is connected and ready for action.
	Connected,
	/// Drive is missing
	Disconnected,
	/// Drive is read-only (unused)
	ReadOnly,
	/// Drive status unknown:
	/// Not a drive, non ATA/LBA drive, or error getting info off it.
	Unknown,
}

enum Errors {
	/// Address mark not found.
	AMNF = 0b0000_0001,
	/// Track zero not found.
	TKZNF = 0b0000_0010,
	/// Aborted command.
	ABRT = 0b0000_0100,
	/// Media change request.
	MCR = 0b0000_1000,
	/// ID not found.
	IDNF = 0b0001_0000,
	/// Media changed.
	MC = 0b0010_0000,
	/// Uncorrectable data error.
	UNC = 0b0100_0000,
	/// Bad Block detected.
	BBK = 0b1000_0000,
}

/// Returns info for all drives. Check these before sending requests.
pub unsafe fn get_drives() -> SVec<DriveInfo, 2> /* or bigger*/ {
	DRIVES.clone()
}

/// Fills up the provided slice with data from disk, starting with `start_sector`
/// This means the slice needs to have a size that's a multiple of 512.
/// # Safety:
/// The contents/existance of a disk to read from is not checked.
pub unsafe fn read_sectors(drive: u8, start_sector: usize, buffer: &mut [u8]) {
	if buffer.len() % 512 != 0 {
		panic!("Buffer must be a multiple of 512 bytes");
	}
	if drive > 1 {
		panic!("No support for more than 2 drives")
	}
	if DRIVES[drive as usize].status != DriveStatus::Connected {
		panic!("Attempt to read non-connected drive")
	}
	while BUSY.load(core::sync::atomic::Ordering::Acquire) {}
	BUSY.store(true, core::sync::atomic::Ordering::Release);
	let lba = buffer.len() / 512;
	let lba48 = DRIVES[drive as usize].lba48;

	select_drive(drive, lba);
	send_lba_and_sector_count(start_sector, lba, lba48);
	wait_till_idle();
	if lba48 {
		COMMAND_REG.write(0x24); // READ SECTORS EXT
	} else {
		COMMAND_REG.write(0x20); // READ SECTORS
	}

	for i in 0..buffer.len() / 512 {
		poll();
		for j in 0..256 {
			let val = DATA_REG.read().to_le_bytes();
			buffer[i * 512 + j * 2] = val[0];
			buffer[i * 512 + j * 2 + 1] = val[1];
		}
		for _ in 0..MAX_ITER / 100 {
			STATUS_REG.read();
		}
	}
	wait_till_idle();
	BUSY.store(false, core::sync::atomic::Ordering::Release);
}

pub unsafe fn write_sectors(drive: u8, start_sector: usize, buffer: &[u8]) {
	if buffer.len() % 512 != 0 {
		panic!("Buffer must be a multiple of 512 bytes");
	}
	if drive > 1 {
		panic!("No support for more than 2 drives")
	}
	if DRIVES[drive as usize].status != DriveStatus::Connected {
		panic!("Attempted write to non-connected disk")
	}
	while BUSY.load(core::sync::atomic::Ordering::Acquire) {}
	BUSY.store(true, core::sync::atomic::Ordering::Release);
	let lba = buffer.len() / 512;
	let lba48 = DRIVES[drive as usize].lba48;

	select_drive(drive, lba);
	send_lba_and_sector_count(start_sector, lba, lba48);
	wait_till_idle();
	if lba48 {
		COMMAND_REG.write(0x34); // WRITE SECTORS EXT
	} else {
		COMMAND_REG.write(0x30) // WRITE SECTORS
	}

	for i in 0..buffer.len() / 512 {
		poll();
		for j in 0..256 {
			let val = u16::from_le_bytes([buffer[i * 512 + j * 2], buffer[i * 512 + j * 2 + 1]]);
			DATA_REG.write(val);
			for _ in 0..MAX_ITER / 100 {
				asm!("jmp no_op", "no_op:", options(nostack, nomem));
			}
		}
		for _ in 0..MAX_ITER / 100 {
			STATUS_REG.read();
		}
	}
	wait_till_idle();
	//Flush cache
	COMMAND_REG.write(0xE7);
	wait_till_idle();
	BUSY.store(false, core::sync::atomic::Ordering::Release);
}

/// Polls the drive until it's idle.
/// End every call to COMMAND_REG with this (after dealing with the result, if applicable) to ensure the next command will be read.
unsafe fn wait_till_idle() {
	loop {
		if STATUS_REG.read() & 0x80 == 0 {
			break;
		}
	}
}

/// Polls the status of selected drive, breaking when it's finished.
unsafe fn poll() {
	//Time to poll (we be singletasking)
	let mut iter = 1;
	loop {
		let status = STATUS_REG.read();
		let bsy = status & 0x80 == 0x80;
		let drq = status & 8 == 8;
		let err = status & 1 == 1;
		let df = status & 0x20 == 0x20;
		if err || df {
			//TODO: error handling
			panic!("Harddisk error")
		} else if !bsy && drq {
			if MAX_ITER < iter {
				MAX_ITER = iter;
			}
			break;
		}
		if iter % MAX_ITER == 0 {
			software_reset();
		}
		if iter % (MAX_ITER * 100) == 0 {
			panic!("Hardrive polling time-out")
		}
		iter += 1;
	}
}

/// Tells the selected disk which sector to start work on on how many sectors
/// # Example:
/// ```
/// //Select master drive
/// DRIVE_HEAD_REG.write(0x40);
/// //Select work sectors
/// send_lba_and_sector_count(start_sector, sectorcount);
/// //Read sectors
/// COMMAND_REG.write(0x24);
/// ```
unsafe fn send_lba_and_sector_count(start_sector: usize, sector_count: usize, lba48: bool) {
	let lba = start_sector.to_le_bytes();
	let sectorcount = sector_count.to_le_bytes();

	if lba48 {
		//high bytes
		SECTOR_COUNT_REG.write(sectorcount[1]);
		LBA_LOW_REG.write(lba[3]);
		LBA_MID_REG.write(lba[4]);
		LBA_HIGH_REG.write(lba[5]);
		//low bytes
		SECTOR_COUNT_REG.write(sectorcount[0]);
		LBA_LOW_REG.write(lba[0]);
		LBA_MID_REG.write(lba[1]);
		LBA_HIGH_REG.write(lba[2]);
	} else {
		SECTOR_COUNT_REG.write(sector_count as u8);
		LBA_LOW_REG.write(lba[0]);
		LBA_MID_REG.write(lba[1]);
		LBA_HIGH_REG.write(lba[2]);
		// lba[3] is sent in select_drive()
	}
}

/// Selects drive based on LBA mode
unsafe fn select_drive(drive: u8, lba: usize) {
	let lba48 = DRIVES[drive as usize].lba48;
	if lba48 {
		DRIVE_HEAD_REG.write(0x40 | (drive << 4))
	} else {
		let lba_high_4 = (lba >> 24) & 0x0F;
		DRIVE_HEAD_REG.write(0xE0 | (drive << 4) | (lba_high_4 as u8));
	}
}

/// Soft reset of the drive
unsafe fn software_reset() {
	DEVICE_CONTROL_REG.write(4);
	DEVICE_CONTROL_REG.write(0);
}
