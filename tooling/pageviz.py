#!/usr/bin/env python3

import struct
import graphviz

# Define constants for x86-64 paging
PAGE_TABLE_ENTRIES = 512
ENTRY_SIZE = 8  # 8 bytes per entry (64-bit)

# Define the bit masks for flag decoding
FLAG_MASKS = {
    'P': 1 << 0,  # Present
    'R/W': 1 << 1,  # Read/Write
    'U/S': 1 << 2,  # User/Supervisor
    'PWT': 1 << 3,  # Page Write-Through
    'PCD': 1 << 4,  # Page Cache Disable
    'A': 1 << 5,  # Accessed
    'D': 1 << 6,  # Dirty (for leaf entries)
    'PS': 1 << 7,  # Page Size (for PDPT/PD entries)
    'G': 1 << 8,  # Global
}

def decode_flags(entry):
    """
    Decodes the flags from a 64-bit paging entry.
    """
    flags = []
    for flag, mask in FLAG_MASKS.items():
        if entry & mask:
            flags.append(flag)
    return ', '.join(flags)

def get_next_table_addr(entry):
    """
    Extracts the physical address of the next level table from a non-leaf entry.
    Bits 12-51 hold the physical address.
    """
    # Mask for bits 12 to 51
    return (entry & 0x000FFFFFFFFFF000)

def parse_paging_structure(dump_bytes, dump_base_phys_addr, pml4_offset_in_dump):
    """
    Parses the memory dump and generates a graphviz Digraph object.

    Args:
        dump_bytes (bytes): The raw memory dump data.
        dump_base_phys_addr (int): The physical address corresponding to the start of the dump.
        pml4_offset_in_dump (int): The byte offset of the PML4 table within the dump.

    Returns:
        graphviz.Digraph: A graph object representing the paging structure.
    """
    # Create a graphviz Digraph with a dark theme
    dot = graphviz.Digraph(comment='x86-64 Paging Structure', graph_attr={
        'bgcolor': '#2d2d2d', 'fontcolor': 'white', 'rankdir': 'LR'
    }, node_attr={
        'fontname': 'Inter', 'fontcolor': 'white', 'shape': 'box', 'style': 'filled', 'color': '#444444'
    }, edge_attr={
        'color': '#888888', 'fontcolor': 'white'
    })

    nodes = {}

    def get_dump_offset(phys_addr):
        """
        Converts a physical address to an offset within the memory dump.
        """
        return phys_addr - dump_base_phys_addr

    def add_table_node(phys_addr, table_type, entries_data):
        """
        Adds a node to the graph for a given paging table.
        """
        node_id = f"table_{phys_addr}"
        if node_id in nodes:
            return node_id

        nodes[node_id] = True

        # Build the HTML label for the table node
        html_label = f'''<<TABLE BORDER="0" CELLBORDER="1" CELLSPACING="0" CELLPADDING="4">
                        <TR><TD COLSPAN="3" BGCOLOR="#3c3c3c"><B>{table_type} @ 0x{phys_addr:x}</B></TD></TR>
                        <TR><TD>Index</TD><TD>Entry Addr</TD><TD>Flags</TD></TR>'''

        for index, entry_phys_addr, decoded_flags in entries_data:
            html_label += f'''<TR><TD>{index}</TD><TD>0x{entry_phys_addr:x}</TD><TD>{decoded_flags}</TD></TR>'''

        html_label += '</TABLE>>'

        dot.node(node_id, label=html_label, _attributes={'fillcolor': '#3a3a3a'})
        return node_id

    def traverse(phys_addr, table_type):
        """
        Recursively traverses the paging structure, creating graph nodes and edges.
        """
        offset = get_dump_offset(phys_addr)
        if offset < 0 or offset + 4096 > len(dump_bytes):
            print(f"Error: Physical address 0x{phys_addr:x} is outside the dump bounds.")
            return None

        current_table_entries_data = []
        next_level_nodes = []
        is_leaf_table = (table_type == 'PT')

        # Read all 512 entries of the current table
        for i in range(PAGE_TABLE_ENTRIES):
            entry_bytes = dump_bytes[offset + i * ENTRY_SIZE : offset + (i + 1) * ENTRY_SIZE]
            entry_value = struct.unpack('<Q', entry_bytes)[0]

            # Check if the entry is present
            if not (entry_value & FLAG_MASKS['P']):
                continue

            decoded_flags = decode_flags(entry_value)

            # Extract the address of the next table or page
            next_phys_addr = get_next_table_addr(entry_value)

            # Add entry to the current table's node data
            current_table_entries_data.append((i, next_phys_addr, decoded_flags))

            # Recursively call for the next level, unless it's a leaf
            # PS (Page Size) flag on PD and PDPT entries indicates a huge page (leaf)
            if not is_leaf_table and not (entry_value & FLAG_MASKS['PS']):
                next_table_type = {'PML4': 'PDPT', 'PDPT': 'PD', 'PD': 'PT'}[table_type]
                next_node_id = traverse(next_phys_addr, next_table_type)
                if next_node_id:
                    next_level_nodes.append((i, next_node_id))

        # Add the current table as a node to the graph
        current_node_id = add_table_node(phys_addr, table_type, current_table_entries_data)

        # Add edges to the next level tables
        for index, next_node_id in next_level_nodes:
            # Use the index as a label for the edge
            dot.edge(current_node_id, next_node_id, label=f'[{index}]')

        return current_node_id

    # Start the traversal from the PML4 table
    pml4_phys_addr = dump_base_phys_addr + pml4_offset_in_dump
    traverse(pml4_phys_addr, 'PML4')

    return dot

if __name__ == '__main__':
    import sys

    if len(sys.argv) != 4:
        print("Usage: python paging_viz.py <mem_dump_file> <start_addr_hex> <pml4_offset_hex>")
        print("Example: python paging_viz.py memory.dump 0x1000 0x2000")
        exit(-1)

    mem_dump_file = sys.argv[1]
    start_addr = int(sys.argv[2], 16)
    pml4_offset = int(sys.argv[3], 16)

    with open(mem_dump_file, 'rb') as f:
        mem_dump = f.read()

        # Generate the graph
        graph = parse_paging_structure(mem_dump, start_addr, pml4_offset)

        # Render to an SVG file
        output_filename = 'paging_structure'
        graph.render(output_filename, format='svg', cleanup=True)

    print(f"Generated SVG file: {output_filename}.svg")