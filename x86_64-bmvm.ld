/* define page size - 4KB for x86_64 */
PAGE_SIZE = 0x1000;
ENTRY(_start)

SECTIONS
{
  . = 0x400000;
  .text : { *(.text*) }

  . = ALIGN(PAGE_SIZE);
  .rodata : { *(.rodata*) }

  . = ALIGN(PAGE_SIZE);
  .data : { *(.data*) }

  . = ALIGN(PAGE_SIZE);
  .bss : { *(.bss*) }

  . = ALIGN(PAGE_SIZE);
  .got : { *(.got*) }

  .bmvm.call.host : ALIGN(4) {
    KEEP(*(.bmvm.call.host));
  }
}