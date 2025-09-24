#!/usr/bin/env bash

set -e

MS_OUT="massif.out.$$"

valgrind -q --tool=massif --massif-out-file="$MS_OUT" "$@"

max_mem=0
while read -r line; do
    if [[ $line =~ mem_heap_B=([0-9]+) ]]; then
        heap=${BASH_REMATCH[1]}
    fi
    if [[ $line =~ mem_heap_extra_B=([0-9]+) ]]; then
        extra=${BASH_REMATCH[1]}
    fi
    if [[ $line =~ mem_stacks_B=([0-9]+) ]]; then
        stacks=${BASH_REMATCH[1]}
        total=$((heap + extra + stacks))
        if (( total > max_mem )); then
            max_mem=$total
        fi
    fi
done < "$MS_OUT"

echo -n "$max_mem"

# rm -f "$MS_OUT"
