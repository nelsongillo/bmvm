# Function Signature
The signature of a function is a combination of the function name, parameters and return type. To be able to successfully
calculate the signature, each parameter and the return type **must** implement the TypeSignature trait, which
defines the signature based on the struct fields, or has a pre-defined signature for a list of selected primitive values.

## TypeSignature
The TypeSignature trait is implemented for following primitives:

`u8`, `u16`, `u32`, `u64`, `u128`, `i8`, `i16`, `i32`, `i64`, `i128`, `f32`, `f64`, `bool`, `char`, `usize`

Following FFI related structs also implement TypeSignature:

`Foreign<T>`, `ForeignBuf`, `Shared<T>`, `SharedBuf`

### User defined structs
TypeSignature can also be implemented for user defined structs via `derive`. Keep in mind, that all struct fields are
required to implement TypeSignature. The struct must either be `repr(C)` or `repr(transparent)`. 

## Signature Computation
The signature is being calculated via the [Djb2](http://www.cse.yorku.ca/~oz/hash.html) hashing algorithm.
The function name, eg "foo" is being used as the baseline of the hash, then all TypeSignatures of the parameters are
included with addition of the index for the parameter. The last information included in the signature hash is the
function return type TypeSignature. All numbers are represented as u64 little endian byte representation.
Example for the function foo:
```rust
fn foo(a: u32, b: char) -> SharedBuf {}
```
the hash would compute as following
```rust
const SIGNATURE_FOO = {
    let mut hasher = Djb2::new();
    hasher.write(b"foo");
    hasher.write(&(0u64).to_le_bytes());
    hasher.write(&<u32 as TypeSignature>::SIGNATURE.to_le_bytes());
    hasher.write(&(1u64).to_le_bytes());
    hasher.write(&<char as TypeSignature>::SIGNATURE.to_le_bytes());
    hasher.write(&<SharedBuf as TypeSignature>::SIGNATURE.to_le_bytes());
    hasher.finish()
};
```
