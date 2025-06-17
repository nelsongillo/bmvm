;-----------------------------------------------------------------------------------------------------------------------
; This code is heavily inspired by the code published at https://wiki.osdev.org/Setting_Up_Long_Mode
;-----------------------------------------------------------------------------------------------------------------------

%define PAGE_SPACE 0x1000
%define IDT_SPACE  0x5000
%define GDT_SPACE  0x6000

%define PAGE_PRESENT    (1 << 0)
%define PAGE_WRITE      (1 << 1)

%define CODE_SEG     0x0008
%define DATA_SEG     0x0010

    org 0x7C00
    global    _start
    section   .text

    bits 16
_start:
    xor ax, ax
    mov ss, ax      ; zero out segment register
    mov sp, _start  ; stack starts below _start

    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    cld

    mov edi, USABLE_SPACE   ; point to the usable space for control structs
    jmp setupLongMode    ; start long mode setup


setupLongMode:
    ; Zero out the 16KiB buffer.
    push di             ; rep stosd modifies di -> push for later use
    mov ecx, 0x1000
    xor eax, eax
    cld
    rep stosd
    pop di

    ; build PML4: es:di points to the table
    ; single entry pointing to PDPT with the present and write flag
    lea eax, [es:di + 0x1000]
    or eax, PAGE_PRESENT | PAGE_WRITE
    mov [es:di], eax

    ; build PDPT with single entry to PD
    lea eax, [es:di + 0x2000]
    or eax, PAGE_PRESENT | PAGE_WRITE
    mov [es:di + 0x1000], eax


    ; build PD with single entry to PT
    lea eax, [es:di + 0x3000]
    or eax, PAGE_PRESENT | PAGE_WRITE
    mov [es:di + 0x2000], eax


    push di
    lea di, [di + 0x3000]               ; point to PT
    mov eax, PAGE_PRESENT | PAGE_WRITE  ; prep eax with present and write flag and 0x0000 addr for all pt entries


; loop over the page table and identity map all entries with PAGE_PRESENT | PAGE_WRITE
.loopBuildPageTable:
    mov [es:di], eax        ; set the entry
    add eax, 0x1000         ; build entry for next page
    add di, 8               ; inc pt index
    cmp eax, 0x200000       ; are we at 2MiB yet -> end
    jb .LoopPageTable

    pop di          ; get original di back

    ; load an empty IDT
    lidt [IDT]

.enterLongMode
    mov eax, 10100000b  ; set the PAE and PGE bits
    mov cr4, eax

    mov edx, edi        ; load PML4 location into CR3
    mov cr3, edx

    mov ecx, 0xC0000080 ; Read from the EFER MSR.
    rdmsr
    or eax, 0x00000100   ; Set the LME bit.
    wrmsr

    mov ebx, cr0                      ; Activate long mode -
    or ebx,0x80000001                 ; - by enabling paging and protection simultaneously.
    mov cr0, ebx

    cli
    lgdt [GDT.Pointer]                ; Load GDT.Pointer defined below.

    jmp CODE_SEG:LongMode             ; Load CS with 64 bit segment and flush the instruction cache


ALIGN 4
IDT:
    .Length       dw 0
    .Base         dd 0

; Function to switch directly to long mode from real mode.
; Identity maps the first 2MiB.
; Uses Intel syntax.

; es:edi    Should point to a valid page-aligned 16KiB buffer, for the PML4, PDPT, PD and a PT.
; ss:esp    Should point to memory that can be used as a small (1 uint32_t) stack


    ; Global Descriptor Table
GDT:
.Null:
    dq 0x0000000000000000             ; Null Descriptor - should be present.

.Code:
    dq 0x00209A0000000000             ; 64-bit code descriptor (exec/read).
    dq 0x0000920000000000             ; 64-bit data descriptor (read/write).

ALIGN 4
    dw 0                              ; Padding to make the "address of the GDT" field aligned on a 4-byte boundary

.Pointer:
    dw $ - GDT - 1                    ; 16-bit Size (Limit) of GDT.
    dd GDT                            ; 32-bit Base Address of GDT. (CPU will zero extend to 64-bit)


[BITS 64]
LongMode:
    mov ax, DATA_SEG
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Blank out the screen to a blue color.
    mov edi, 0xB8000
    mov rcx, 500                      ; Since we are clearing uint64_t over here, we put the count as Count/4.
    mov rax, 0x1F201F201F201F20       ; Set the value to set the screen to: Blue background, white foreground, blank spaces.
    rep stosq                         ; Clear the entire screen.

    ; Display "Hello World!"
    mov edi, 0x00b8000

    mov rax, 0x1F6C1F6C1F651F48
    mov [edi],rax

    mov rax, 0x1F6F1F571F201F6F
    mov [edi + 8], rax

    mov rax, 0x1F211F641F6C1F72
    mov [edi + 16], rax

    jmp Main.Long                     ; You should replace this jump to wherever you want to jump to.