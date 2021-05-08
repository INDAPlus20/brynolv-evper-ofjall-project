use super::partitions::Partition;
use crate::svec::SVec;

pub struct FileInfo {
	pub name: SVec<char, 12>,
	pub size: usize,
	pub is_directory: bool,
}

type Path = SVec<char, 128>;

pub unsafe fn initialize() {
	todo!()
}

pub unsafe fn write_file(path: &Path, data: &[u8]) {
	todo!()
}

pub unsafe fn read_file(path: &Path, buffer: &mut [u8]) {
	todo!()
}

pub unsafe fn get_file_info(path: &Path) -> FileInfo {
	todo!()
}

pub unsafe fn list_entries(directory_path: &Path) -> SVec<FileInfo, 32> {
	todo!()
}
