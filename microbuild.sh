#!/usr/bin/env bash

benches=("kvm" "load" "parse" "link" "run")
links=("links1" "links8" "links16" "links32" "links64" "links128")
output="bench/binaries/micro"
mkdir -p "$output"

# Loop over all combinations
for b in "${benches[@]}"; do
  for l in "${links[@]}"; do
    cargo build --package micro --release --features="$b,$l"
    cp ./target/release/micro "$output/micro-$b-$l"
  done
done
