//! ## Background: PostgreSQL vs Rust on Memory Management
//!
//! *If you're familiar with writing Postgres extensions in C, most of this will
//! be redundant information to you, but unless you're an expert in Rust as
//! well, you probably should not skip it.*
//!
//! Postgres performs most of it's memory management using various memory
//! allocation pools known as "memory contexts", rather than directly using
//! system allocation APIs like `malloc` and `free`. The major difference here
//! is that PG will often "reset" a memory context, automatically freeing all
//! memory allocated within it. Memory contexts are often short-lived, and get
//! reset frequently, in an attempt to defeat memory leaks.
//!
//! ### Aside: Why PostgreSQL does things this way
//!
//! Postgres' system is designed to make memory leaks more unlikely, and as we
//! noted above, it does this at the consequence of making temporal memory
//! safety violations (e.g. use after frees) more likely. While to a Rust
//! programmer, this may sound like a... very backwards choice -- after all,
//! Rust explicitly made the opposite choice (leaking memory is safe in Rust,
//! after all), the reality is that Postgres' system makes a fair bit of sense
//! for a codebase which uses setjmp/longjmp-style[^sjlj] error handling.
//!
//! [^sjlj]: Technically, Postgres uses `sigsetjmp` and `siglongjmp` rather than
//!     the plain versions.
//!
//! Concretely, setjmp and longjmp offer a rudamentary form of unwinding, one
//! where destructors of values on the stack are not generally run (and even if
//! they were, support for defining such destructors is not even possible in
//! standard C).
//!
//! That means that a system like PG, even if it could be rewritten today, has
//! limited options for cleanup, and none of them are very good:
//!
//! - Implementing destructors manually by adding hooks to a list which gets run
//!   prior to `longjmp` is somewhat traditional (on some targets an automatic
//!   variant of this is even how C++/Rust-style unwinding is implemented by the
//!   compiler), but is quite painful to do by hand, especially in plain C,
//!   which does not even have closures. Not to mention, it would be
//!   considerably slower and likely would serve as more of a barrier to
//!   compiler optimizations.
//!
//! - Implementing (or integrating) a full garbage collection system would solve
//!   this, but can be invasive, slow, and makes it hard to reason both
//!   performance and resource usage. Garbage collectors often need careful
//!   tuning for their given workload, and I'm sure that this would be true for
//!
//! - Avoiding setjmp/longjmp for error handling would solve the issue, but adds
//!   a lot of bookkeeping. Notably, the Postgres' error reports contain
//!   additional data beyond fairly rich
//!
//!
//!
//!  It's also worth noting, it would likely be far harder to integrate this
//!   solution with Rust code, so while it may sound cleaner, it would be
//!   unlikely t
//!
//!  are a bit like like panics do not run destructors, nor do
//!
//! While this seems like an odd choice in 2023, it actually makes quite a bit
//! of sense when
//!
//! Postgres is a C library is using error handling based around
//! `setjmp`/`longjmp`, which does not afford efficient mechanisms for running
//! cleanup code. That is: without some form of automatic memory managment like
//! this, memory leaks seem almost unavoidable.
//!
//! However, this also points at why it is less critical for Rust extensions to
//! use this system, and why it's reasonable for us to disgard certain pieces of
//! advice present in the Postgres' documentation. Rust has automatic cleanup at
//! the end of functions. Not only that, PGRX carefully translates[^1] between
//! Postgres ERROR conditions (which include longjmps) and rust panics -- this
//! is done bidirectionally, so that errors which travel over Rust stack frames
//! always do so using `panic` unwinding, and errors which travel over .
//!
//! [^1]: If we call a PG function which reports an `ERROR` (or anything else
//!     that gets it to longjmp), we convert it to a `panic` before immediately
//!     after it crosses into Rust, and if a panic unwinds all the way out of
//!     Rust, we convert it back to the appropriate postgres error, that is, it
//!     started as an error report caught by rust, then we remember and resore
//!     the error report information, making the longjmp->panic->longjmp
//!     round-trip lossless (in cases where nothing catches and manipulates the
//!     error in the middle).
//!
//!     This is handled automatically by macros like `#[pg_extern]`, as well as
//!     all the wrappers which call into various `pg_sys` functions -- you don't
//!     have to do anything for it to work, even in cases where you manually
//!     call `pg_sys` functions.
//!
//!
//! the fact that PG utilizes a form of automatic memory management is
//!
//!  as they would otherwise be a large problem for Postgres C code
//!
//! although it has the side-effect of making temporal memory safety violations
//! (like use-after-frees) more likely.
//!
//! While PG's concerns may seem outdated, the fact that the project uses a
//! setjmp/longjmp-based system for error handling.
//!
//! This is less of a concern for Rust; unlike in C we can take advantage of
//! Rust's RAII/`Drop` (destructors) to ensure cleanup code is run and memory
//! leaks are avoided -- this is uses `sigsetjmp`/`siglongjmp` for error
//! handling,
//!
//!
//! Memory contexts support a "reset" operation, which bulk-frees any memory
//! allocated from within that context in a single efficient operation. Memory
//! contexts can also be deleted (which resets them first), but for simplicity
//! this document will consider the "lifetime" of a memory context to end when
//! the memory context is reset. The distinction is not relevant for most Rust
//! code, so we're going to gloss over it.
//!
//! Memory contexts also may have child memory contexts. When a memory context
//! is reset, any children it has are also reset (and deleted). It is possible
//! to change the parent from one context to another (for example, moving to a
//! more long-lived context as a way to extend the lifetime of allocations from
//! the child), although doing so is fairly advanced.
//!
//! Several global variables exist which point to different memory contexts. The
//! (probably) most important of these is the "current memory context"
//! ([`pg_sys::CurrentMemoryContext`]).
//!
//! ### The Current Memory Context
//!
//! The `CurrentMemoryContext` is a global variable in the Postgres C API which
//! points to a memory context that should be used for allocations. In practice,
//! this is used in at least two ways:
//!
//! 1. It functions as an implicit additional parameter in various PG APIs which
//!    directly allocate and memory from it -- Postgres has many functions which
//!    return newly allocated objects, and they they usually allocate them out
//!    of the current memory context (unless that function's documentation
//!    states otherwise).
//!
//!     It is fairly uncommon to find Postgres APIs which accept a memory
//!     context as one of the parameters.
//!
//!     It is only in rare cases that they allow the user to explicitly pass a
//!     memory context in as a parameter.
//!
//! 2. It may be set to a short-lived context which should be used for
//!    short-lived allocations which are not returned.
//!
//! These are somewhat in conflict with eachother -- for example, where should
//! functions which return memory do for temporary allocation? The
//!
//! It is possible that with deeper understanding of the PostgreSQL source,
//! better answers to this would be obvious, but
//!
//!  We generally solve this by:
//! -
//!
//! ### Aside: Malloc Mimics: `palloc` and friends
//!
//! These have some subtle semantic differences from the variants you might be
//! familiar with from C.
//!
//! - [`pg_sys::palloc`] and [`pg_sys::palloc0`] (zeroing variant), allocate out
//!   of the current memory context.
//!
//! - [`pg_sys::palloc_aligned`] is a PG16-only API, and is like `palloc` allows
//!   allocating memory for types that require a higher alignment (see notes on
//!   alignment below).
//!
//! - [`pg_sys::repalloc`] reallocates memory, like C's realloc. Note that it
//!   does *not* use the current memory context, but instead the memory context.
//!   If memory as allocated using `palloc_aligned`, it preserves the requested
//!   alignment (see notes on alignment below).
//!
//! - [`pg_sys::pfree`] frees memory allocated from Postgres.
//!
//! #### Alignment
//!
//! PG's allocation functions return memory aligned the needs of the types it
//! uses. Concretely, it returns memory aligned to [`pg_sys::MAXIMUM_ALIGNOF`].
//! This is the maximum of `align_of::<c_long>()`, `align_of::<c_longlong>()`
//! and `align_of::<f64>()`. In practice, usually 8 (on 32 bit targets it is
//! usually 4, but `pgrx` does not really support these).
//!
//! This is sadly insufficient for many Rust types. For example, `i128` and
//! `u128` often require 16 byte alignments. So you should probably not allocate
//! your Rust types . Doing so is likely to panic. Note that while we could
//! manually align the resulting pointer from `palloc`, doing so yields a
//! pointer that cannot be `pfree`d or `repalloced`. In other words, most (all?)
//! case where this would be acceptable don't need to allocate the memory from a
//! memory context in the first place.
//!
//! That said, PG16 adds a `palloc_aligned` that solves this problem, and PGRX
//! should use it when necessary if PG16 is being targetted. Note that PG16 is
//! so new that, at the time of writing this (September 2023), it is still
//! pre-release.
//!
