use core::sync::atomic::AtomicBool;

use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

use crate::svec::SVec;

// Assuming "typical" ports, LBA48
const IO_BASE_PORT:u16 = 0x1F0;
static mut DATA_REG:Port<u16>=Port::new(IO_BASE_PORT);
static mut ERROR_REG:PortReadOnly<u16>=PortReadOnly::new(IO_BASE_PORT+1);
static mut FEATURES_REG:PortWriteOnly<u16>=PortWriteOnly::new(IO_BASE_PORT+1);
static mut SECTOR_COUNT_REG:Port<u8>=Port::new(IO_BASE_PORT+2);//Actually u16, but low and high needs to be sent separately.
static mut LBA_LOW_REG:Port<u8>=Port::new(IO_BASE_PORT+3);//same
static mut LBA_MID_REG:Port<u8>=Port::new(IO_BASE_PORT+4);// with these 2
static mut LBA_HIGH_REG:Port<u8>=Port::new(IO_BASE_PORT+5);
static mut DRIVE_HEAD_REG:Port<u8>=Port::new(IO_BASE_PORT+6);
static mut STATUS_REG:PortReadOnly<u8>=PortReadOnly::new(IO_BASE_PORT+7);
static mut COMMAND_REG:Port<u8>=Port::new(IO_BASE_PORT+7);
const CONTROL_BASE_PORT:u16 = 0x3F6;
static mut ALT_STATUS_REG:PortReadOnly<u8>=PortReadOnly::new(CONTROL_BASE_PORT);
static mut DEVICE_CONTROL_REG:PortWriteOnly<u8>=PortWriteOnly::new(CONTROL_BASE_PORT+0);
static mut DRIVE_ADRESS_REG:PortReadOnly<u8>=PortReadOnly::new(CONTROL_BASE_PORT+0);

/// Is the driver/disk busy?
/// TODO: make this per disk
static BUSY:AtomicBool=AtomicBool::new(false);

pub unsafe fn initialize() {
    DEVICE_CONTROL_REG.write(0);
}

pub struct DriveInfo {
    pub drive: u8,
    pub status: DriveStatus
}

pub enum DriveStatus {
    Connected,
    Disconnected,
    ReadOnly
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
    BBK = 0b1000_0000
}

struct Register {
    offset: u8,
    direction: Direction,
    param_size_28:u8,
    param_size_48:u8,
    control:bool
}

enum Direction {
    R,
    W,
    RW
}

pub unsafe fn get_drives() -> SVec<DriveInfo, 4> /* or bigger*/ {
    todo!()
}

pub unsafe fn read_sectors(drive: u8, start_sector: usize, buffer: &mut [u8]) {
    if buffer.len() % 512 != 0 {
        panic!("Buffer must be a multiple of 512 bytes");
    }
    if drive>1{
        panic!("No support for more than 2 drives")
    }
    let sectorcount=(buffer.len()/512).to_le_bytes();
    let lba = start_sector.to_le_bytes();
    while BUSY.load(core::sync::atomic::Ordering::Acquire) {};
    BUSY.store(true, core::sync::atomic::Ordering::Release);
    
    DRIVE_HEAD_REG.write(0x40|(drive<<4));
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
    COMMAND_REG.write(0x24);//READ SECTORS EXT

    
    for i in 0..buffer.len()/512 {
        poll();
        for mut j in 0..256{
            let val=DATA_REG.read().to_le_bytes();
            buffer[i*512+j]=val[0];
            j+=1;
            buffer[i*512+j]=val[1];
        }
    }
}

unsafe fn poll(){
    //Time to poll (we be singletasking)
    loop {
        let status = STATUS_REG.read();
        let bsy = status&0x80==0x80;
        let drq = status&8==8;
        let err = status&1==1;
        let df = status&0x20==0x20;

        if err||df {
            //TODO: error handling
            panic!("Harddisk error")
        } else if !bsy&&drq{
            break;
        }
    }
}

pub unsafe fn write_sectors(drive: u8, start_sector: usize, buffer: &[u8]) {
    if buffer.len() % 512 != 0 {
        panic!("Buffer must be a multiple of 512 bytes");
    }

    todo!()
}

unsafe fn software_reset(){
    DEVICE_CONTROL_REG.write(4);
    DEVICE_CONTROL_REG.write(0);
}