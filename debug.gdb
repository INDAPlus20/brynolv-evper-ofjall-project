# GDB script which will automatically attempt to connect to QEMU and load symbol files.
# Currently, this always loads symbol files from the debug binary, as the release binary
# contains no debug information. There may be times when debugging the release and manully loading symbols
# may be preferred.

# Connect to the qemu instance.
target remote tcp::1234

# Load debug symbols from the binary.
# Currently, the kernel is loaded into memory by the bootloader
# at the same addresses that the debug symbols point to.
# If this is ever changed, an offset must be specified.
symbol-file ./target/x86_64-unknown-caesarsallad/debug/brynolv-evper-ofjall-project

# Set a breakpoint at the entry function.
# This will pause execution after the call to _start().
break _start

# As qemu will initially be paused, continue execution.
# Execution should stop at the call to _start().
continue
