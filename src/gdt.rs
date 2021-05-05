use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};


static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

/// Initializes the Global Descriptor Table.
///
/// # Safety
///
/// This should not be called if another call to this function has not yet returned.
pub unsafe fn initialize() {
    let code_segment = GDT.add_entry(Descriptor::kernel_code_segment());
    let data_segment = GDT.add_entry(Descriptor::kernel_data_segment());
    GDT.load();
    x86_64::instructions::segmentation::load_ss(data_segment);
    x86_64::instructions::segmentation::set_cs(code_segment);
}
