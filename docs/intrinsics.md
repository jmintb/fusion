# Intrinsics

Intrinsics are called just like functions, but are handled as a special case by the compiler. This
is to handle cases that the language can not express directly. Over time the aim is to move 
them to be implemented in normal "userland" fusion code.



The intrinsic are documented with a function signature to make it clear what the input and output
types are. There are no underlying functions, the compiler will emit a set of instructions to
perform the target operation within the function calling the intrinsic. The `intrinsic` keyword 
used below tells the compiler that this is an intrinsic and not an actual function.

## Memory operations

> [!WARNING]
> **These operations can break the static guarantees enforced by the compiler, as they manipulate memory directly.**
> Furthermore keep in mind that they are experimental and need to have a more careful review before 
> used for anything serious. Typical error cases have not been considered or dealt with yet.
> Eventually using them will require entering a manual block, where it is up to the user to ensure
> the compilers invariants are upheld. For now there is no distinction between manual and non manual
> code blocks so use this functionality with care.
> [!WARNING]

There are a number of intrinsics to support operating on memory directly. They are heavily inspired
by Rust's intrinsics.


```rust

// Writes a number of bytes to a location in memory.
// value is treated as an array of bytes.
// destination denotes the memory location to write to.
// length specifies the number of bytes to write.
intrinsic fn write_bytes(owned value: ptr, owned destination: ptr, owned length: integer);

// Add a signed offset to the base pointer producing a new pointer. 
// This can potentially wrap.
intrinsic fn pointer_from_offset(owned base_pointer: ptr, owned offset: integer) -> ptr;


```

> [!IMPORTANT]
> Memory related intrinsics will have some changes to the naming and parameter types once the 
> builtin types for arrays and integers are refactored. This is expected to happen in the very 
> near future.


