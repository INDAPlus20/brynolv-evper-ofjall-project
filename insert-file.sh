#!/bin/bash

mcopy -i out/boot-uefi-brynolv-evper-ofjall-project.fat test.txt ::test.txt
dd if=out/boot-uefi-brynolv-evper-ofjall-project.fat \
of=out/boot-uefi-brynolv-evper-ofjall-project.img bs=1K count=2K \
conv=notrunc seek=17
