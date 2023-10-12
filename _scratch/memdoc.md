# PGRX Memory Contexts

## Background: Postgres Memory Management

Postgres performs most of it's memory management using various memory allocation pools known as "memory contexts", rather than directly using system allocation APIs like `malloc` and `free`.

Memory contexts support a "reset" operation, which bulk-frees any memory allocated from within that context in a single efficient operation. Memory contexts can also be deleted (which resets them first), but for simplicity this document will consider the "lifetime" of a memory context to end when the memory context is reset. The distinction is not relevant for most Rust code, so we're going to gloss over it.

Memory contexts also may have child memory contexts. When a memory context is reset, any children it has are also reset (and deleted). It is possible to change the parent from one context to another (for example, moving to a more long-lived context as a way to extend the lifetime of allocations from the child), although doing so is fairly advanced.

Several global variables exist which point to different memory contexts. The (probably) most important of these is the "current memory context" ([`pg_sys::CurrentMemoryContext`]). This is the "default context" which is used by APIs that don't.

### Malloc Mimics: `palloc` and friends

`pg_sys::palloc`, [`pg_sys::palloc0`] allocate out of the current memory context. This memory is aligned to `pg_sys`

### Sad truth about alignment

Memory allocated from PG's memory contexts is aligned to `pg_sys::MAXIMUM_ALIGNOF` by default. This constant is less than might be expected -- it is usually 8 (when either `align_of::<f64>()` or `align_of::<c_long_long>()` is 8), but it sometimes is 4. It is never more than 8 though, even on platforms where C's `alignof(maxalign_t)` would be larger.

On it's own, this sounds fine -- it is not that difficult to implement aligned allocation on top of unaligned, and PGRX could easily provide helpers for this... But there's a wrinkle: Situations exist where for whatever reason you *must* use memory allocated from a memory context ("palloced memory") for the input or output of some operation. For such cases, it is possible that PG wants use the pointer with `repalloc` or `pfree`, or something else which will inspect the allocation header.

The good news is that this is rarely a problem. Usually PG doesn't

An easy solution to this only exists in pg16, which adds `palloc_aligned`.
Unfortunately, PG16 is so new that at the time of writing this... it has not
yet been released. Prior to that, the solution to this problem is likely
context-dependent, but the easy answer might be adding `#[repr(C, packed)]`
to whatever is putting you over the alignment max.
