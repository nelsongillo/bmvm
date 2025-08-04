# FFI
Here is a general overview of the involved registers regarding hyper- and upcalls:

## General
To transport function parameter and return values, we define a transport structure, fittingly named `Transport`.
This struct contains two `u64` fields:
```rust
struct Transport {
    primary: u64,
    secondary: u64,
}
```
* The `primary` field is used to transport either offset ptr to a shared structure, or for primitive types such as integer,
floats and chars (given they fit into the u64).
* The `secondary` is an optional field, which is only used for byte buffer, where `primary` contains the pointer and secondary
contains the capacity. If `secondary == 0`, the field will be interpreted as empty, as zero-sized buffer are permitted.

## Register
The concept is independent of the calling direction (host to guest/guest to host). If necessary, `rbx` will contain
the function signature to call, and `r8`,`r9` contain the transport structure:
* Call
    * RBX: Function signature
    * R8: Transport.Primary
    * R9: Transport.Secondary
* Return
    * R8: Transport.Primary
    * R9: Transport.Secondary

