CC = $(TOOLPREFIX)gcc
AS = $(TOOLPREFIX)gas
LD = $(TOOLPREFIX)ld
OBJCOPY = $(TOOLPREFIX)objcopy
OBJDUMP = $(TOOLPREFIX)objdump
CFLAGS = -fno-pic -static -fno-builtin -fno-strict-aliasing -O2 -Wall -MD -ggdb -m32 -Werror -fno-omit-frame-pointer
#CFLAGS = -fno-pic -static -fno-builtin -fno-strict-aliasing -fvar-tracking -fvar-tracking-assignments -O0 -g -Wall -MD -gdwarf-2 -m32 -Werror -fno-omit-frame-pointer
CFLAGS += $(shell $(CC) -fno-stack-protector -E -x c /dev/null >/dev/null 2>&1 && echo -fno-stack-protector)
ASFLAGS = -m32 -gdwarf-2 -Wa,-divide
# FreeBSD ld wants ``elf_i386_fbsd''
LDFLAGS += -m $(shell $(LD) -V | grep elf_i386 2>/dev/null | head -n 1)
arch ?= x86
target ?= i686-unknown-linux-gnu
assembly_source_files := $(wildcard src/*.S)
assembly_object_files := $(patsubst src/%.S, \
    %.o, $(assembly_source_files))
linker_script := kernel.ld

ifndef CPUS
CPUS := 2
endif
QEMUOPTS = -drive file=fs.img,index=1,media=disk,format=raw -drive file=cofflos.img,index=0,media=disk,format=raw -m 512 $(QEMUEXTRA) -d guest_errors #-d int -no-reboot

qemu: fs.img cofflos.img
	qemu-system-i386  $(QEMUOPTS) -monitor stdio

rust_os := target/$(target)/debug/librv6.a

# Build the bootloader block
bootblock: src/bootasm.S src/bootmain.rs
	rustc -C relocation-model=static -C opt-level=s -C debuginfo=0 --crate-type=staticlib  -Z no-landing-pads --emit=obj src/bootmain.rs
	$(CC) $(CFLAGS) -fno-pic -nostdinc -I. -c src/bootasm.S
	$(LD) $(LDFLAGS) -N -e start -Ttext 0x7C00 -o bootblock.o bootasm.o bootmain.o
	$(OBJCOPY) -S -O binary -j .text bootblock.o bootblock
	./sign.pl bootblock

initcode: src/initcode.S
	$(CC) $(CFLAGS) -nostdinc -I. -c src/initcode.S
	$(LD) $(LDFLAGS) -N -e start -Ttext 0 -o initcode.out initcode.o
	$(OBJCOPY) -S -O binary initcode.out initcode
	$(OBJDUMP) -S initcode.o > initcode.asm

entryother: src/entryother.S
	$(CC) $(CFLAGS) -fno-pic -nostdinc -I. -c src/entryother.S
	$(LD) $(LDFLAGS) -N -e start -Ttext 0x7000 -o bootblockother.o entryother.o
	$(OBJCOPY) -S -O binary -j .text bootblockother.o entryother

cofflos.img: bootblock kernel fs.img
	dd if=/dev/zero of=cofflos.img count=10000
	dd if=bootblock of=cofflos.img conv=notrunc
	dd if=kernel of=cofflos.img seek=1 conv=notrunc

fs.img: mkfs TODO.md Cargo.toml
	./mkfs fs.img TODO.md Cargo.toml

mkfs: src/mkfs.c src/fs.h
	gcc -Werror -Wall -o mkfs src/mkfs.c

entry.o: src/entry.S
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o entry.o src/entry.S

kernel: cargo $(rust_os) entry.o entryother kernel.ld initcode
	@ld -n --gc-section -T kernel.ld -o kernel entry.o $(rust_os) -b binary initcode entryother

-include *.d

cargo:
	cargo rustc --target $(target) -- -Z no-landing-pads --crate-type=staticlib -C relocation-model=static
#cargo rustc -- -C relocation-model=static --crate-type=staticlib -Z no-landing-pads


clean:
	rm -f *.tex *.dvi *.idx *.aux *.log *.ind *.ilg \
	*.o *.d *.asm *.sym vectors.S bootblock entryother \
	initcode initcode.out kernel xv6.img fs.img kernelmemfs mkfs \
	.gdbinit \
	$(UPROGS)
