#include "mmu.h"
#include "types.h"
#include "memlayout.h"
unsigned int entrypgdir[];

__attribute__((__aligned__(PGSIZE)))
unsigned int entrypgdir[1024] = {
  // Map VA's [0, 4MB) to PA's [0, 4MB)
  [0] = (0) | PTE_P | PTE_W | PTE_PS,
  // Map VA's [KERNBASE, KERNBASE+4MB) to PA's [0, 4MB)
  [KERNBASE>>PDXSHIFT] = (0) | PTE_P | PTE_W | PTE_PS,
};

