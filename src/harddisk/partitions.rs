use crate::svec::SVec;


pub struct Partition {
    drive: u8,
    partition_guid: [u8; 16],
    start_sector: usize,
    sector_count: usize,
    name: SVec<char, { 72 / 2 }>
}

impl Partition {
    pub fn sector_count(&self) -> usize {
        self.sector_count
    }
}

pub unsafe fn initialize() {
    todo!()
}

pub unsafe fn list_partitions() -> SVec<Partition, 16> {
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
