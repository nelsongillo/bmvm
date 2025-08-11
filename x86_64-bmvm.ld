/* define page size - 4KB for x86_64 */
PAGE_SIZE = 0x1000;
ENTRY(_start)

SECTIONS
{
    . = 0x400000;
    .text : AT(0) {
        *(.text*)
    }

    . = ALIGN(PAGE_SIZE);
    .rodata : AT(ADDR(.text) + SIZEOF(.text)) {
        *(.rodata*)
    }

    . = ALIGN(PAGE_SIZE);
    .data : AT(ADDR(.rodata) + SIZEOF(.rodata)) {
        *(.data*)
    }

    . = ALIGN(PAGE_SIZE);
    .bss : AT(ADDR(.data) + SIZEOF(.data)) {
        *(.bss*)
    }

    . = ALIGN(PAGE_SIZE);
    .got : AT(ADDR(.bss) + SIZEOF(.bss)) {
        *(.got*)
    }


    /* VMI related section */
    .bmvm.vmi.debug : {
        KEEP(*(.bmvm.vmi.debug));
    }

    /* TODO: specify PH_NOTE */
    .bmvm.vmi.expose (NOLOAD) : {
        KEEP(*(.bmvm.vmi.expose));
    }

    /* TODO: specify PH_NOTE */
    .bmvm.vmi.expose.calls (NOLOAD) : {
        KEEP(*(.bmvm.vmi.expose.calls));
    }

    /* TODO: specify PH_NOTE */
    .bmvm.vmi.host (NOLOAD) : {
        KEEP(*(.bmvm.vmi.host));
    }
}