pub mod fat32;
mod partitions;
mod pata;

pub unsafe fn initialize() {
	pata::initialize();
	partitions::initialize();
	fat32::initialize();
}
