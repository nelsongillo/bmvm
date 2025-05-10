SECTIONS {
    .metadata.kvm : ALIGN(4) {
        KEEP(*(.metadata.kvm));
    }
}
