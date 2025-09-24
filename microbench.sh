#!/usr/bin/env bash

micros=$1
binaries=$2

HOSTS=()
# Find all micro files
while IFS= read -r -d $'\0' file; do
    HOSTS+=("$file")
done < <(find "$micros" -maxdepth 1 -type f -executable -print0)

GUESTS=()
# Find all guests files
while IFS= read -r -d $'\0' file; do
    GUESTS+=("$file")
done < <(find "$binaries" -maxdepth 1 -type f -executable -print0)

# Loop over all combinations
for h in "${HOSTS[@]}"; do
  for g in "${GUESTS[@]}"; do
        # Extract last 5 chars
        suff_h="${h: -5}"
        suff_g="${g: -5}"

        if [[ "$suff_h" == "$suff_g" ]]; then
          ./target/release/benchy --runtime native --mode exec --warmup 128 --iters 1024 --output ./data --file "$h" --args "$g"
        fi
  done
done
