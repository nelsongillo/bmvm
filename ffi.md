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

## Memory Safety
When the peer calls a function with multiple parameters, a wrapper struct is generated.
```rust
// A function signature like this one generates a parameter struct
fn foo(a: u32, b: bool, c: Foreign<Foo>) {}

struct ParamsFoo {
  a: u32,
  b: bool,
  c: Foreign<Foo>,
}
```
The struct is being allocated in the calling memory region, and the pointer is passed to the callee. The callee then has
to dereference the struct pointer, and pass the fields to the function. For primitive types this is not a problem, as
they all implement `Copy` and `Clone`. This is not the case for fields of type `Foreign<T>` and `ForeignBuf`, as they 
implement a custom drop logic, where the underlying memory is deallocated using the caller allocator. Simply copying
these fields might cause double drops and use after free when the parameter struct goes out of scope, as these types are
only glorified pointers.
A solution to this problem copy the fields and prevent the parameter struct from calling drop on its fields:
```rust
/// Unpacking implementation by ParamsFoo
unsafe impl Unpackable for ParamsFoo {
  type Output = (u32, Foreign<Foo>);
  /// copy each field using `ptr::read` and return the owned values
  unsafe fn unpack(this: *const Self) -> Self::Output {
    let a = unsafe { core::ptr::read(&(*this).a) };
    let b = unsafe { core::ptr::read(&(*this).b) };
    return (a, b);
  }
}

impl<U: Unpackable> Foreign<ParamsFoo> {
    pub unsafe fn unpack(self) -> U::Output {
        // ManuallyDrop to prevent automatic dropping
        let this = ManuallyDrop::new(self);
        // get the raw pointer to the underlying value
        let ptr = this.get_ptr();
        // unpack the fields from the struct (copies the values)
        let output = unsafe { U::unpack(ptr) };
        // due to the copy of the underlying struct fields, the original value can be deallocated
        // without dropping (prevents double drop)
        this.manually_dealloc();

        output
    }
}

impl wrapper_foo() {
  // parse the transport struct to a ParamsFoo ptr
  let params = Foreign<ParamsFoo>::from_transport()?;
  // unpack the parameter
  let (a, b, c) = params.unpack();
  // call foo with owned parameter
  foo(a, b, c);
}
```