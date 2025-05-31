SECTIONS
{
    . = 0x400000; /* Start at address 0 */
    .text : { *(.text*) }
    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    .bss : { *(.bss*) }
    .bmvm.call.host
}