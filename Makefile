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
.DEFAULT_GOAL := kernel

QEMU = qemu-system-i386
GDBPORT = $(shell expr `id -u` % 5000 + 25001)
# QEMU's gdb stub command line changed in 0.11
QEMUGDB = $(shell if $(QEMU) -help | grep -q '^-gdb'; \
	then echo "-gdb tcp::$(GDBPORT)"; \
	else echo "-s -p $(GDBPORT)"; fi)

.gdbinit: .gdbinit.tmpl
	sed "s/localhost:1234/localhost:$(GDBPORT)/" < $^ > $@

qemu-gdb: fs.img cofflos.img .gdbinit
	@echo "*** Now run 'gdb'." 1>&2
	$(QEMU) -serial mon:stdio -nographic $(QEMUOPTS) -S $(QEMUGDB)


ifndef CPUS
CPUS := 2
endif
QEMUOPTS = -drive file=fs.img,index=1,media=disk,format=raw -drive file=cofflos.img,index=0,media=disk,format=raw -m 512 $(QEMUEXTRA) -d guest_errors #-d int -no-reboot

qemu: fs.img cofflos.img
	qemu-system-i386  $(QEMUOPTS) -monitor stdio

qemu-dbg: fs.img cofflos.img
	#qemu-system-i386  $(QEMUOPTS) -nographic -d int -no-reboot
	qemu-system-i386  $(QEMUOPTS) -d int -no-reboot -serial mon:stdio -nographic

qemu-console: fs.img cofflos.img
	qemu-system-i386 -nographic $(QEMUOPTS) -serial mon:stdio

qemu-net: fs.img cofflos.img
	qemu-system-i386 -nographic $(QEMUOPTS) -serial mon:stdio -device rtl8139

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

entryother: src/entryother.S
	$(CC) $(CFLAGS) -fno-pic -nostdinc -I. -c src/entryother.S
	$(LD) $(LDFLAGS) -N -e start -Ttext 0x7000 -o bootblockother.o entryother.o
	$(OBJCOPY) -S -O binary -j .text bootblockother.o entryother

cofflos.img: bootblock kernel fs.img
	dd if=/dev/zero of=cofflos.img count=10000
	dd if=bootblock of=cofflos.img conv=notrunc
	dd if=kernel of=cofflos.img seek=1 conv=notrunc

fs.img: lib/simple_fs/src/bin.rs lib/simple_fs/src/lib.rs README
	dd if=/dev/zero of=fs.img bs=512 count=1000
	cargo run --manifest-path lib/simple_fs/Cargo.toml -- fs.img README

mkfs: lib/simple_fs/src/bin.rs lib/simple_fs/src/lib.rs
	cargo build --manifest-path lib/simple_fs/Cargo.toml

entry.o: src/entry.S
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o entry.o src/entry.S

vectors.o: src/vectors.S
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o vectors.o src/vectors.S

trapasm.o: src/trapasm.S
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o trapasm.o src/trapasm.S

swtch.o: src/swtch.S
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o swtch.o src/swtch.S

kernel: cargo $(rust_os) entry.o entryother kernel.ld initcode vectors.o trapasm.o swtch.o
	@ld -n --gc-section -T kernel.ld -o kernel entry.o vectors.o trapasm.o swtch.o $(rust_os) -b binary initcode entryother
	$(OBJDUMP) -t kernel | sed '1,/SYMBOL TABLE/d; s/ .* / /; /^$$/d' > kernel.sym

-include *.d

cargo:
	cargo rustc --target $(target) -- -Z no-landing-pads --crate-type=staticlib -C relocation-model=static -C debuginfo=2 -C target-feature=-mmx,-sse

clean:
	rm -f *.tex *.dvi *.idx *.aux *.log *.ind *.ilg \
	*.o *.d *.asm *.sym vectors.S bootblock entryother \
	initcode initcode.out kernel xv6.img fs.img kernelmemfs mkfs \
	.gdbinit \
	$(UPROGS)
