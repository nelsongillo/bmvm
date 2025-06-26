# Outline

## Introduction
* Problem statement
* thesis structure

# Background + Related Work
* Virtualization types
* ELF -> ELKVM
* WASM -> Hyperlight WASM

## Design
* Problem Statement and Requirements
### Architecture on component level 
### VM initializaiton
* Longmode setup (Host vs Guest)
* ELF loading (Host vs Guest)
### Function Calling
* Linking
* Host to Guest (Blocking/Interrupt)
* Guest to Host

## Implementation
* VM initialization
* Linking

## Benchmarks
* bootup time
* different configs (eg msg size, function parameter size, ..)
* execution heavy workloads (comparison with WASM)
* IO heavy workloads (comparison with WASM)

## Limitations
## Future Works
## Conclusion