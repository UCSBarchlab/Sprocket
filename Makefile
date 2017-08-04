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
host_target ?= i686-unknown-linux-gnu
target ?= i686-sprocket
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

qemu-gdb: fs.img sprocket.img .gdbinit
	@echo "*** Now run 'gdb'." 1>&2
	$(QEMU) -serial mon:stdio -nographic $(QEMUOPTS) -S $(QEMUGDB)


ifndef CPUS
CPUS := 2
endif
QEMUOPTS = -drive file=fs.img,index=1,media=disk,format=raw -drive file=sprocket.img,index=0,media=disk,format=raw -m 512 $(QEMUEXTRA) -d guest_errors -device rtl8139,netdev=unet,mac='C0:FF:EE:12:34:56' -netdev tap,id=unet,helper=/usr/lib/qemu/qemu-bridge-helper# -object filter-dump,netdev=unet,id=netdev,file=dump.pcap
#-d int -no-reboot

qemu: fs.img sprocket.img
	qemu-system-i386  $(QEMUOPTS) -monitor stdio

qemu-dbg: fs.img sprocket.img
	#qemu-system-i386  $(QEMUOPTS) -nographic -d int -no-reboot
	qemu-system-i386  $(QEMUOPTS) -d int -no-reboot -serial mon:stdio -nographic

qemu-console: fs.img sprocket.img
	qemu-system-i386 -nographic $(QEMUOPTS) -serial mon:stdio

qemu-net: fs.img sprocket.img
	qemu-system-i386 -nographic $(QEMUOPTS) -serial mon:stdio

# -netdev bridge,id=br0 -object filter-dump,netdev=br0,id=br0,file='dump.pcap'

rust_os := target/$(target)/debug/libsprocket.a

# Build the bootloader block
bootblock: src/bootasm.S src/bootmain.rs
	rustc -C relocation-model=static -C opt-level=s -C debuginfo=0 --crate-type=staticlib  -Z no-landing-pads --emit=obj src/bootmain.rs
	$(CC) $(CFLAGS) -fno-pic -nostdinc -I. -c src/bootasm.S
	$(LD) $(LDFLAGS) -N -e start -Ttext 0x7C00 -o bootblock.o bootasm.o bootmain.o
	$(OBJCOPY) -S -O binary -j .text bootblock.o bootblock
	./sign.pl bootblock

sprocket.img: bootblock kernel fs.img
	dd if=/dev/zero of=sprocket.img count=10000
	dd if=bootblock of=sprocket.img conv=notrunc
	dd if=kernel of=sprocket.img seek=1 conv=notrunc

fs.img: lib/simple_fs/src/bin.rs lib/simple_fs/src/lib.rs README.md index.html
	dd if=/dev/zero of=fs.img bs=512 count=1000
	cargo run --manifest-path lib/simple_fs/Cargo.toml --target $(host_target) -- fs.img README.md index.html

mkfs: lib/simple_fs/src/bin.rs lib/simple_fs/src/lib.rs
	cargo build --target $(host_target) --manifest-path lib/simple_fs/Cargo.toml

entry.o: src/entry.S src/param.h
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o entry.o src/entry.S

vectors.o: src/vectors.S
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o vectors.o src/vectors.S

trapasm.o: src/trapasm.S
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o trapasm.o src/trapasm.S

swtch.o: src/swtch.S
	gcc -m32 -gdwarf-2 -Wa,-divide -c -o swtch.o src/swtch.S

kernel: cargo $(rust_os) entry.o kernel.ld vectors.o trapasm.o swtch.o
	@ld -n --gc-section -T kernel.ld -o kernel entry.o vectors.o trapasm.o swtch.o $(rust_os) -b binary
	$(OBJDUMP) -t kernel | sed '1,/SYMBOL TABLE/d; s/ .* / /; /^$$/d' > kernel.sym

cargo:
	xargo rustc --target $(target) -- -Z no-landing-pads --crate-type=staticlib -C relocation-model=static -C debuginfo=2 -C target-feature=-mmx,-sse

clean:
	rm -rf *.tex *.dvi *.idx *.aux *.log *.ind *.ilg \
	*.o *.d *.asm *.sym vectors.S bootblock \
	kernel sprocket.img fs.img kernelmemfs mkfs \
	.gdbinit target \
	$(UPROGS)
