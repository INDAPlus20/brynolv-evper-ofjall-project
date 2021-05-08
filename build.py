# This script is used to build the project into a bootable disk image.
# See https://docs.rs/bootloader/0.10.1/bootloader/ for more information.

import subprocess
import sys
import json
import os

# Currently, the default is DEBUG executables.
debug = True
# Wether or not to run the binary in qemu after building it.
# Note that qemu needs to be installed for this option.
run = False
# Wether qemu should wait for a connection from gdb before
# starting execution or not.
gdb = False

def print_usage():
    print("usage: python build.py [options...]")
    print("  available options: release, run, gdb, help")
used_options = []
for arg in sys.argv[1:]:
    if arg == 'help':
        print("usage: python build.py [options...]")
        print("  options:")
        print("     release")
        print("         Build the kernel with 'cargo build --release'.")
        print("         This enables optimizations and doesn't emit debug info.")
        print("     run")
        print("         After building the disk image, run it in qemu.")
        print("         qemu needs to be installed for this to work.")
        print("         To install, follow instructions at https://www.qemu.org/download/.")
        print("     gdb")
        print("         Tells qemu to wait for a connection from gdb at port 1234")
        print("         before starting execution.")
        print("         Must be used in conjunction with 'run'.")
        print("     help")
        print("         Prints this help screen, and then exits.")
        exit(0)
    elif arg == 'release':
        if 'release' not in used_options:
            debug = False
            used_options.append('release')
        else:
            print("Error: Option 'release' specified twice")
            print_usage()
            exit(1)
    elif arg == 'run':
        if 'run' not in used_options:
            run = True
            used_options.append('run')
        else:
            print("Error: Option 'run' specified twice")
            print_usage()
            exit(1)
    elif arg == 'gdb':
        if 'gdb' not in used_options:
            gdb = True
            used_options.append('gdb')
        else:
            print("Error: Option 'gdb' specified twice")
            print_usage()
            exit(1)
    else:
        print("Error: Unknown argument '" + arg + "'")
        print_usage()
        exit(1)

if gdb and not run:
    print("Error: Option 'gdb' specified but not 'run'")
    print("       'gdb' must always be used in conjunction with 'run'.")
    exit(1)

# We need to parse the project metadata to find the local path of the 'bootloader' dependency.
# For this command to succeed, this script needs to have been called from
# the project root directory.
# Else, the return code should be non-zero.
result = subprocess.run(["cargo", "metadata"], stdout=subprocess.PIPE)
if result.returncode != 0:
    # We only captured stdin, so stderr should have been printed
    exit(1)
# result.stdout is a byte array, so we need to decode it using UTF-8
metadata = json.loads(result.stdout.decode("utf-8"))

# Method to find `bootloader` source path adapted from
# https://docs.rs/bootloader-locator/0.0.4/src/bootloader_locator/lib.rs.html#11-40

dep_id = None
root_name = metadata['resolve']['root']
for node in metadata['resolve']['nodes']:
    found = False # used for breaking the outer loop from the inner loop
    if node['id'] == root_name:
        for dep in node['deps']:
            if dep['name'] == 'bootloader':
                dep_id = dep['pkg']
                found = True
                break
    if found:
        break

if dep_id == None:
    print("No dependency 'bootloader' found")
    exit(1)

# As we found the dependency, 'packages' should ALWAYS contain an entry with id 'dep_id'.
for package in metadata['packages']:
    if package['id'] == dep_id:
        dep_path = os.path.dirname(package['manifest_path'])

project_path = os.getcwd()
manifest_path = os.path.join(project_path, 'Cargo.toml')
binary_path = os.path.join(
    project_path,
    'target',
    'x86_64-unknown-caesarsallad',
    'debug' if debug else 'release',
    'brynolv-evper-ofjall-project'
)
target_path = os.path.join(project_path, 'target')
out_path = os.path.join(project_path, 'out')

# Check if the out path exists.
# If it doesn't, create it
if not os.path.exists(out_path):
    os.mkdir(out_path)

# First, build the project to a normal binary
build_command = ['cargo', 'build']
if not debug:
    build_command.append('--release')
result = subprocess.run(build_command)
if result.returncode != 0:
    exit(1)
    

# Second, create the bootable disk image
# See https://docs.rs/bootloader/0.10.1/bootloader/ for more information
result = subprocess.run([
    'cargo', 'builder',
    '--kernel-manifest', manifest_path,
    '--kernel-binary', binary_path,
    '--target-dir', target_path,
    '--out-dir', out_path], cwd=dep_path)
if result.returncode != 0:
    exit(1)

# There should now be 4 different disk images in ./out:
# - boot-bios-brynolv-evper-ofjall-project.img
# - boot-uefi-brynolv-evper-ofjall-project.efi
# - boot-uefi-brynolv-evper-ofjall-project.fat
# - boot-uefi-brynolv-evper-ofjall-project.img
# What these files are used for is described at https://docs.rs/bootloader/0.10.1/bootloader/

if run:
    run_command = ['qemu-system-x86_64', '-bios', 'bios.bin', 'out/boot-uefi-brynolv-evper-ofjall-project.img']
    if gdb:
        run_command += ['-s', '-S']
    subprocess.run(run_command)
