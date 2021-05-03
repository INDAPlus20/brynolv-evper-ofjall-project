use crate::svec::SVec;


pub unsafe fn initialize() {
    todo!()
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

pub unsafe fn get_drives() -> SVec<DriveInfo, 4> /* or bigger*/ {
    todo!()
}

pub unsafe fn read_sectors(drive: u8, start_sector: usize, buffer: &mut [u8]) {
    if buffer.len() % 512 != 0 {
        panic!("Buffer must be a multiple of 512 bytes");
    }

    todo!()
}

pub unsafe fn write_sectors(drive: u8, start_sector: usize, buffer: &[u8]) {
    if buffer.len() % 512 != 0 {
        panic!("Buffer must be a multiple of 512 bytes");
    }

    todo!()
}
