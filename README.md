# Caesarsallad

[Link to repository](https://github.com/INDAPlus20/brynolv-evper-ofjall-project)

## Goal

The goal of this project is to create a simple bare-metal file editor for the AMD64 architecture in `Rust`.

### Target Features

#### **MVP**

- PS/2 keyboard support
- ASCII support
- Backspace + inserting characters

#### **Usable**

- PATA hard drive interface
- Fat32 file system support
- Opening/Saving file.txt for appending
- - When appending, the last X rows of the original file will be shown above the editing area

#### **Good enough**

- Cursor movement
- Choose file path
- - Path handling
- Edit entire file
- List directory

#### **Extra features**

- Partial unicode support
- - Collapsing unknown multi-byte character to single "unknown character" graphic
- Multiple files open in multiple tabs
- - Possibly with splitting the viewport into multiple subviews
- - Diff view

## Naming Conventions

### Commits

Commits should be named in the past tense - e.g. "Fixed bla bla"

### Pull Requests

Pull requests should be named in the present tense - e.g. "Fixes bla bla"

### Issues

Issue titles should either be a short description of an issue (not the Github-issue kind) or bug,
or be named in the imperative mood - e.g. "Keyboard not registered" or "Fix keyboard not being registrered"

## Feasibility

An `MVP` is feasible. Reaching the `Usable` should be feasible, depending on how difficult implementing the file system driver is.
Any stage above `Usable` may or may not be feasible, depending on the scope of the previous stages.

## Planning

### Week 0

- Writing this specification
- Setting up development environment

### Week 1

- `MVP`

### Week 2

- PATA
- Start Fat32-driver

### Week 3

- Finish Fat32-driver

### Week 4

- Finish rest of `Usable`
- `Good enough` (should be relatively simple)

## Project difficulty motivation

We believe that this will reach and exceed the level of difficulty needed, as extensive knownledge about the hardware used is required.
This can be ascertained by reviewing this specification and referencing the appropriate pages on [the osdev wiki](https://wiki.osdev.org).

The difficulty of this project is increased by some team members not being comfortable with the language used (`Rust`).

## Building

Building the project uses a Python script called `build.py`, which can be found in the root of the project.
How to execute Python scripts can vary between systems. This section will use the syntax `./build.py`, but many systems require `python build.py` or `python3 build.py`. Note that `build.py` cannot be run with Python 2.

`build.py` has a help screen available with `./build.py help`.

### Dependencies

`build.py` requires the following dependencies:

- Python 3
- Cargo
- - Is a part of the Rust installation available from (the rust website)[https://www.rust-lang.org/tools/install].
- The `rust-src` component
- - Intalled with `rustup component add rust-src`
- QEMU (to run the project)
- - Can be downloaded from [the QEMU website](https://www.qemu.org/download/)

Both `cargo` and `qemu-system-x86_64` (part of the QEMU installation) must be available in `PATH`.

### Compiling

Simply running `./build.py` will compile the project.
Running `./build.py release` will pass the `--release` flag to `cargo`, enabling optimizations
and disabling debug information.

### Running

Running `./build.py run` will first compile the program, then run the compiled disk image in QEMU.
Note that this requires QEMU to be installed. `release` can be added to build the kernel in release mode.

### Debugging

Running `./build.py run gdb` will compile and run the project in QEMU. QEMU will then pause execution and wait for a debugger to connect. Any debugger that supports the GDB Remote Protocol can be used.

The debugger suggested is GDB. GDB can be found in many package repositories, and can also be compiled and installed [from source](https://www.gnu.org/software/gdb/download/).

A script for GDB is supplied in the root of the project, called `debug.gdb`. Running `gdb -x debug.gdb` **while a QEMU session is running** will start GDB and continue execution until the call to `_start()`, the entry point of the kernel.
Note that `debug.gdb` loads the debug symbols from the `debug` build, and if QEMU is running a release build, the symbols loaded will not be correct. As such, `debug.gdb` should never be run with a QEMU session started by `./build run gdb release`.
