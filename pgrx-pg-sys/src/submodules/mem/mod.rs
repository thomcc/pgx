// TODO, it's not really great that this is in `pgrx-pg-sys`. It's somewhat hard
// to avoid at the moment, unfortunately.
//! # PGRX Memory Management
//!
//! *Note: while this module is defined in `pgrx_pg_sys`, conceptually is is
//! more in line with `pgrx`. It currently must be in `pg_sys` due to certain
//! trait implementations. Note that *
//!
//! This module reconciles the (otherwise incompatible) memory management
//! strategies of Postgres and Rust. It allows safe code to manage memory in
//! certain ways, and allows unsafe code to build sound abstractions.
//!
//! ## Usage guidelines
//!
//! These APIs want you to use them a specific way. Unsafe code which uses them
//! in other ways is likely to be unsound. We have made it so safe code cannot
//! misuse these APIs, but it's basically trivial for unsafe code to do so --
//! even easier to use them right in some cases. So please use them carefully.
//!
//! While it's certain that there are cases where the guidelines must be broken,
//! be certain that you fully understand them first, including why they are (and
//! why they must be!) so different from the way C Postgres code would be
//! written.
//!
//! ### Current Context Usage
//!
//! #### `MemCx::make_current`
//!
//! Never use the `CurrentMemoryContext` global as a way to communicate the
//! allocator to use between different Rust functions. An explicit `&MemCx`
//! should always be passed into Rust functions which need to allocate memory
//! for return or storage
//!
//! ```ignore
//! // Bad (indicating via global):
//! let result = memcx.make_current(|| something.some_rust_function());
//! // Good (passing it in explicitly):
//! let result = something.some_rust_function(memcx);
//! ```
//!
//! More concretely, all functions where the lifetime must be represented, or
//! where the caller may care which context some memory was allocated in should
//! represent that by taking that context as an argument, rather than by having
//! the caller set the current context prior to the call.
//!
//! ---
//!
//! The closure provided to `make_current` should be very small, ideally just
//! ```ignore
//! // Good: single small call
//! let result = memcx.make_current(|| unsafe {
//!     pg_sys::something(args, here)
//! });
//! // Good: it's fine to also call simple Rust functions
//! // like constructing wrapper types
//! let result = memcx.make_current(|| unsafe {
//!     let ptr = pg_sys::something_else(args, here);
//!     PBox::from_raw_in(ptr, memcx)
//! });
//! // Bad: doe
//! let result = memcx.make_current(|| unsafe {
//!     let ptr = pg_sys::something_else(args, here);
//!     PBox::from_raw_in(ptr, memcx)
//! });
//! ```
//!
//! pg_detoast_datum_packed
//!
//! There are likely exceptions to the "never" here, but they should be very
//! rare. Note that the overhead of shuffling around `CurrentMemoryContext` is
//! too small to be measured compared to calling postgres functions (so
//! performance would be a very misguided reason to combine `make_current`)
//!
//! -
//!
//! `CurrentMemoryContext` hs
//!
//! `pgrx` code should almost never use the implicit `CurrentMemoryContext`, as
//! it makes it impossible to assign a lifetime to allocated memory. We solve
//! this by having functions which need to allocated memory accept the `&MemCx`
//! to use as a type parameter.
//!
//! A `&MemCx` can be produced for some contexts safely. Producing such a
//! reference is known as "borrowing the memory context", and in general, the
//! rule is that it's UB to reset or delete a context while it is borrowed.
//!
//! At the moment, `CurrentMemoryContext` and `TopMemoryContext` are the only
//! ones which may be borrowed from safe code. This is not intended to be a
//! closed set -- it's likely that there are more contexts that are safe to
//! borrow from.
//!
//!

//! Instead, functions should accept the memory context as a `&MemCx` parameter.
//!
//!
//! Note that it is forbidden to reset or delete a memory context while a
//! `&MemCx` exists for it.
//!
//!
//!
//!
//! When calling Rust functions, always pass in the memory context which should
//! be used rather than implicitly using the current context. For example:
//!
//! It's a bit complex. There are a few pieces of advice here:
//! 1. Use the Rust allocator for anything that doesn't need to be allocated in
//!    a memory context
//! 2.
//!
//!     ```ignore
//!     // Bad:
//!     ctx.make_current(|| something.do_things());
//!     // Good:
//!     something.do_things(ctx);
//!     ```
//!     The closures  `make_current`  
//!     ```ignore
//!     // Bad:
//!     ctx.make_current(|| something.do_things());
//!     // Good:
//!     something.do_things(ctx);
//!     ```
//!
//!
//!  is that most Rust code should use the Rust allocator wherever it does not
//! need to be using Postgres-managed memory.
//!
//! ## Things that can't be expressed
//!
//! Some patterns common in PostgreSQL's C code are impossible to express in a
//! safe Rust API.
//!
//! A notable one is the pattern of a data structure holding a memory context
//! used to allocate its internal data structures (and in many cases, itself as
//! well). This is a self-referential borrow, which is something that Rust
//! cannot express with lifetimes, you must use raw pointers indefinitely.
//!
//! ### Guideline Rationale
//!
//! This API is useful both to safe and unsafe code, but it mainly exists to
//! allows unsafe code to express postgres APIs in a way that is sound and
//! memory-safe while avoiding leaks and needless copies.
//!
//! But that only works if the unsafe code uses the code in this module as
//! intended, e.g. folloing these guidelines.
//!
//! In particular the APIs are opinionated about a few things, and it's actually
//! fairly easy for unsafe code to misuse them, and not much we can do to stop
//! them -- making APIs resistant to incorrect unsafe is somewhatq impossible,
//! after all.
//!
//! But in particular, several of the things here are a little unintuitive, and
//! even contrary to how C code does it. This is not because the C Postgres code
//! is wrong to do it the way it does, but because the set of needs and
//! constraints for C and Rust are fundamentally different here -- Postgres C
//! uses contexts the way they do to avoid memory leaks even in the face of
//! `longjmp`, and Rust uses contexts the way it does because it must do so to
//! know which lifetime to attach to allocations (and also becuase we can rely
//! on destructors for cleanup, since `longjmp`s from C are translated into
//! panics in Rust, and vice-versa).
//!
//!
//!
//!  while it is entirely reasonable for carefully-considered code to decide
//! that one or another of the
//!
//!
//! is impossible for , even in the case where they merely
//!
//! The advice above is somewhat contrary to the general advice give to Postgres
//! extensions written in C, which usually includes:
//! - Always use the contexts for memory allocation, rather than other
//!   allocators.
//! - When interacting with external resources which must be allocated outside
//!   of the contexts, use a a reset hook to ensure it's cleaned up.
//! - Avoid passing a context into functions, and just use the
//!   `CurrentMemoryContext` global.
//! - And so on.
//!
//! We tell you to use the Rust allocator where possible, pass an explicit
//! memory context into everything, and
//!
//! There are several reasons for this, but the main reason is that PG's C
//! doesn't have a way to run cleanup when error conditions cause it to longjmp
//! across many stack frames. We arrange so that Rust frames traversed using
//! panic, and never `longjmp`. This is non-optional because it is UB to longjmp
//! over arbitrary Rust stack frames, but it has the very pleasant side effect
//! of allowing us to rely on rust
//!
//!
//!  in PG's C, code, memory leaks are a much larger concern due to
//! `siglongjmp`, which occurs on errors, not offering a chance to perform
//! cleanup.
//!
//! We carefully translate between PG's `siglongjmp`s and Rust `panic`s at every
//! boundary point between the languages (preserving information as much as
//! possible), so that we never `panic` across C code, or longjmp across Rust
//! code. This means that
pub mod raw;
