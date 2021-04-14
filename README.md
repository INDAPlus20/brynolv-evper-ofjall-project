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
or be named in the future tense - e.g. "Keyboard not registered" or "Fix keyboard not being registrered"

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