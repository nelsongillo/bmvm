#!/usr/bin/env python3

alignment_map = {
    '16b' : 16,
    '4kib': 4 * 1024,
    '2mib': 2 * 1024 * 1024,
    '1gib': 1 * 1024 * 1024 * 1024,
}

def is_aligned(value, alignment):
    if alignment not in alignment_map:
        raise ValueError(f"Invalid alignment: {alignment}")

    alignment_in_bytes = alignment_map[alignment]
    return value % alignment_in_bytes == 0


def main():
    while True:
        # User input
        value_input = input("Value (hex/dec): ")
        alignment_input = input("Alignment (4KiB, 2MiB, 1GiB): ").lower()

        if value_input.startswith("0x"):
            value = int(value_input, 16)
        else:
            value = int(value_input)

        # Check alignment
        print(is_aligned(value, alignment_input))

if __name__ == "__main__":
    main()
