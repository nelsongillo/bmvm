/* define page size - 4KB for x86_64 */
PAGE_SIZE = 0x1000;
ENTRY(_start)


PHDRS
{
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
    bss PT_LOAD;
    got PT_LOAD;
    note PT_NOTE;
}

SECTIONS
{
    . = 0x400000;
    .text : AT(0) {
        *(.text*)
    } :text

    . = ALIGN(PAGE_SIZE);
    .rodata : AT(ADDR(.text) + SIZEOF(.text)) {
        *(.rodata*)
    } :rodata

    . = ALIGN(PAGE_SIZE);
    .data : AT(ADDR(.rodata) + SIZEOF(.rodata)) {
        *(.data*)
    } :data

    . = ALIGN(PAGE_SIZE);
    .bss : AT(ADDR(.data) + SIZEOF(.data)) {
        *(.bss*)
    } :bss

    . = ALIGN(PAGE_SIZE);
    .got : AT(ADDR(.bss) + SIZEOF(.bss)) {
        *(.got*)
    } :got


    .bmvm.vmi.debug (NOLOAD): {
        KEEP(*(.bmvm.vmi.debug));
    } :note

    .bmvm.vmi.expose : {
        KEEP(*(.bmvm.vmi.expose));
    } :note

    .bmvm.vmi.expose.calls : {
        KEEP(*(.bmvm.vmi.expose.calls));
    } :note

    .bmvm.vmi.host : {
        KEEP(*(.bmvm.vmi.host));
    } :note
}