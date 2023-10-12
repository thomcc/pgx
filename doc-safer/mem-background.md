# Appendix: PostgreSQL vs Rust on Memory Management

> **Note**
>
> This is something of an appendix to the memory documentation, but it's
> somewhere between a mind-dump of my thoughts on how Postgres and Rust memory
> management interact, and the documentation that I wish I could have read when
> I started working on PGRX's memory safety (or really, on PGRX in general).
>
> At first I imagined it would be part our rustdoc documentation, but it seems
> fairly unsuitable for that -- too many tangents, opinions, and rarely-relevant
> details have made it into the text. While a ruthless editing pass could have
> cleaned this up, I felt it might better serve as a separate file mainly for
> internal documentation.

Postgres performs most of it's memory management using various memory allocation
pools known as "memory contexts", rather than directly using system allocation
APIs like `malloc` and `free`. The major difference here is that PG will often
"reset" a memory context, automatically freeing all memory allocated within it.
Memory contexts are often short-lived, and get reset frequently, in an attempt
to defeat memory leaks.

Memory contexts support a "reset" operation, which bulk-frees any memory
allocated from within that context in a single efficient operation. Memory
contexts can also be deleted (which resets them first), but for simplicity this
document will consider the "lifetime" of a memory context to end when the memory
context is reset. The distinction is not relevant for most Rust code, so we're
going to gloss over it.

Memory contexts also may have child memory contexts. When a memory context is
reset, any children it has are also reset (and deleted). It is possible to
change the parent from one context to another (for example, moving to a more
long-lived context as a way to extend the lifetime of allocations from the
child), although doing so is fairly advanced.

Several global variables exist which point to different memory contexts. The
(probably) most important of these is the "current memory context"
([`pg_sys::CurrentMemoryContext`]).

### The Current Memory Context

The `CurrentMemoryContext` is a global variable in the Postgres C API which
points to a memory context that should be used for allocations. In practice,
this is used in at least two ways:

1. It functions as an implicit additional parameter in various PG APIs which
   directly allocate and memory from it -- Postgres has many functions which
   return newly allocated objects, and they they usually allocate them out of
   the current memory context (unless that function's documentation states
   otherwise).

    It is fairly uncommon to find Postgres APIs which accept a memory context as
    one of the parameters.

    It is only in rare cases that they allow the user to explicitly pass a
    memory context in as a parameter.

2. It may be set to a short-lived context which should be used for short-lived
   allocations which are not returned.

These are somewhat in conflict with eachother -- for example, where should
functions which return memory do for temporary allocation?

In PGRX, our rough design philosophy here is:
- Leaking should be avoided internally as much as possible. This means that the
  answer to that question is "it doesn't really matter if it `pfree`s all those
  temporaries, so it will".

- Exposing APIs which leak palloced memory is sometimes okay or unavoidable, but
  we make it easier to use short-lived contexts for this. In other words, some
  of the borrowed context APIs are very much like `bumpalo::Bump` or
  `typed_arena::TypedArena`

- Lots of the time, you should just use owned Rust types.



It is possible that with deeper understanding of the PostgreSQL source, better
answers to this would be obvious, but

 We generally solve this by:
-


### Aside: Malloc Mimics: `palloc` and friends

Postgres's global allocation functions have some subtle semantic differences
from the variants you might be familiar with from C (despite being mostly
familiar-looking). I was surprised by the behavior of these, so here's a short
guide. There are some others functions (and other ways the functions do not
quite behave like C's), but these are the big ones.

- `pg_sys::palloc`, `pg_sys::palloc0` (the zeroing variant),
  `pg_sys::palloc_extended` (the "extra flags" variant), and
  `pg_sys::palloc_aligned` the (PG16-only aligned variant) all allocate out of
  the current memory context.

    These are fine, but note that in cases where you already have a memory
    context (and would have to assign it to `CurrentMemoryContext` for the
    duration of the call), the variants which take explicit context arguments
    should be somewhat preferred (e.g. `pg_sys::MemoryContextAlloc`,
    `pg_sys::MemoryContextAllocZero`, `pg_sys::MemoryContextAllocExtended`, and
    so on).

    > **Warning**
    >
    > Despite the name, `pg_sys::MemoryContextAllocZeroAligned` is not an
    > explicit context+aligned+zeroed allocation function, but instead a
    > micro-optimized version of `pg_sys::MemoryContextAllocZero` for use when
    > the size happens to be compatible with some memset function of PG's. PGRX
    > code should not use this generally (and likely has no real reason to),
    > unless we're very certain the usage correct.

- `pg_sys::pfree` frees memory allocated from Postgres. It does not matter which
  memory context it came from, since this is determined from the allocation
  automatically (if it helps, you can think of it as being looked up using the
  in the allocation's header).

- `pg_sys::repalloc` reallocates memory, similar to C's realloc. Note that it
  does *not* use the current memory context, but instead the memory context
  where the memory was originally allocated.

    If memory as allocated using PG16's `palloc_aligned`, then `repalloc` will
    preserve that requested alignment (see notes on alignment below).

- `pg_sys::palloc_huge`/`pg_sys::MemoryContextAllocHuge` is needed to allocate
  more than 1GB of memory at once (the other functions will error in such
  cases).

    We currently don't expose this or use it automatically. It's unclear what
    the right behavior is here (doing so automatically seems dangerous, but the
    alternative seems tricky to reconcile with our traits), so we ignore the
    issue, which seems to work so far.

Also worth noting that unlike Rust, zero-sized allocations from a memory
contexts are real allocations (of 0 bytes of memory), and even must be freed.
They aren't entirely useless either, as (as with any allocation) resizing a
zero-sized allocation with `repalloc` will still happen in the context where it
was initially allocated. I could imagine this helps avoid needing to hold onto a
memory context pointer in some cases, but Rust code (with the new APIs) will
usually have this around anyway, so it's not that useful

### Aside: Aligned Allocation Tips

PG's allocation functions return memory aligned the needs of the types it uses.
Concretely, it returns memory aligned to [`pg_sys::MAXIMUM_ALIGNOF`]. This is
the maximum of `align_of::<c_long>()`, `align_of::<c_longlong>()` and
`align_of::<f64>()`. In practice, usually 8 (on 32 bit targets it is usually 4,
but `pgrx` does not really support these).

Unfortunately, this is insufficient for many Rust types. For example, `i128` and
`u128` often require 16 byte alignments. So you should probably not allocate
your Rust types inside Postgres if you can avoid it.

If you're staring at the `pg_sys::palloc_aligned` and
`pg_sys::MemoryContextAllocAligned` APIs, these were added in PG16, which is so
bleeding-edge that it's not even released yet (as of September 2023). We don't
expose a simple API for this (only non-simple ones full of caveats), because
adding APIs which tie you to such a modern version go against the goal of making
it straighforward to target all versions of PG that PGRX supports. Like, it's
not even out yet -- it's too new to consider that.

If you really need to allocate use a memory context to allocate higher-aligned
memory, what you should do is situational, and may require . Here's some options
(or: "the new version of PGRX is asserting about my type's alignment, what do I
do?")

- If you can just allocate it with the Rust allocator (Box, Vec, etc), this is
  what you should do. (Often this is more viable than you might expect)

- If the reason you need the aligned allocation to be performed in whichever
  specific context you need is that you have no other way to ensure it gets
  cleaned up at the right time, then perhaps allocating with the Rust allocator,
  and then installing a context hook for cleanup is the best option.

- If it does not need to be a pointer that that Postgres manages for you (that
  is, it never needs to access the allocation header of the pointer, for example
  with `repalloc` or `pfree`), then manually aligning the pointer is viable.
  PGRX has code to do this for you based on a given `core::alloc::Layout` (and
  could even use the PG16 apis with it when they are available, although perhaps
  this would mask bugs).

    Note: It seems like manually-aligned pointers like this should probably be
    passed around as `internal` if they must go through SQL at all, but it's
    possible there are other options -- this is at least what folks seem to do
    in C extensions that need this (such as `pg_roaringbitmap`).

    A slight variant of this is allocating a possibly-misaligned pointer (with
    the spare space needed for alignment), and passing the misaligned pointer
    around, but aligning it to the required alignment whenever it is used (for
    example, converting to a reference).

- In some cases using packed structs or unaligned reads is viable (or even
  appropriate), but it's highly error-prone and extremely easy to do wrong. Be
  careful if you go this route.

- If none of these are the case, reach out on Discord and we can give you
  suggestions or further information on one of the options.

If you aren't sure if this is a concern, it only is if `align_of::<YourType>()`
is more than 8 on any of the targets that you care about.



### Aside: Why PostgreSQL does things this way

Or: "Postgres says you should *always* use these contexts, we say you should
avoid them much of the time, is Rust just 'special'?"

> **Note**
>
> This contains a fair amount of speculation / opinion, although it explains the
> rationale behind some of PGRX's design decisions.

Postgres' system is designed to make memory leaks more unlikely, and as we noted
above, it does this at the consequence of making temporal memory safety
violations (e.g. use after frees) more likely. To a Rust programmer, this may
sound like a... very backwards choice -- after all, Rust explicitly made the
opposite choice (leaking memory is safe in Rust, after all).

For a long time I just thought this design might be historical baggage, but
while working on this I realized it's because of how PG implements error
handling (`sigsetjmp`/`siglongjmp`). Critically, this also explains a lot of the
recommendations they made which previously may have seemed overly paranoid about
memory leaks.

Anyway, enough beating around the bush: Postgres uses setjmp/longjmp-style
functions for error handling, which give rudamentary form of unwinding (so is at
least superficially similar to `panic` in Rust or `throw` in C++). I called it
"rudamentary" because (in most implementations), control flow is transfered
*directly* from the code that throws (the longjmper) to the code that catches
(the setjmper) *without* running anything in-between. In contrast, Rust
panicking and C++ throwing use more sophisticated unwinding schemes which does
not transfer control directly, but instead works its way up the stack, calling
destructors of any types present on the

Because setjmp/longjmp do not run destructors, the options for how to perform
cleanup are... limited, and mostly quite painful. It's obviously impossible to
change the existing code but in a hypothetical world, here are some alternatives
they could have chose

1. Manually implement destructors, by adding the cleanup functions to a list of
   things to run when throwing. This is insanely tedious in C, which doesn't
   have closures.

2. Use real unwinding, but without compiler support it would basically have the
   same issues as the manual destructors. I *suppose* they could avoid that by
   switching to C++ but not only is that suggestion unreasonable, C++ is...
   somewhat than C about what you are allowed to do with types which have
   destructors (which, are required for this to be useful), so it's likely that
   this isn't even remotely practical. Even if it is, it seems unlikely that an
   extension system would be vibrant in a C++-implemented postgres, due to C++'s
   significant ABI stability issues.

3. Implement or integrate with a garbage collector of some sort.

4. Avoid setjmp/longjmp for error handling and return errors by hand. The way
   this is integrated with everything else makes me think this would be a huge
   redesign, it also easily could have performance problems and be tedious, but
   if it would take a redesign then who knows, maybe it would be designed not to
   be that way.

5. And so on. Note that none of these would be very easy to use from Rust other
   than 4, which is also the most significant change (and one which is very easy
   to see why it was not chosen). In particular, both use of a GC or use of C++
   would be especially nightmarish for PGRX.

Those all kinda suck, but without a way to perform cleanup in the case of
setjmp/longjmp error handling (and given how PG uses errors, these are
frequent), memory leaks will be unavoidable. When it's considered from that
perspective, the current system seems fine -- it's basically a very simple and
efficient "garbage collector", just one with dangerously sharp edges.

Finally, we're at the point where it startes to become clear why using the
memory contexts doesn't matter for Rust extensions so much, and why it's
reasonable for PGRX (and PGRX users) to disgard certain pieces of advice present
in the Postgres' documentation.

Rust has automatic cleanup at the end of functions. Not only that, PGRX
carefully translates[^1] between cases where PG's longjmps and rust panics --
this is done bidirectionally, so that PG's errors which travel over Rust stack
frames always do so using `panic` unwinding, and panics that escape Rust are
turned into longjmps.

This means there is no situation where a longjmp ever jumps over a rust stack
frame containing a Rust destructor. Doing so (force-unwinding over a
non-plain-old-frame) is actually considered UB in Rust, and could even break
some of the code in the Rust standard library.

Because of this, Rust destructors on the stack are always executed (because
panic unwinding will do so). This means cleanup for Rust code works pretty
simply and as Rust developers would expect, even in the face of errors. This
means Rust extensions don't really have to worry about leaking to *quite* the
same degree as C extension devs -- we actually still have to worry about it
quite a bit in PGRX's design and internals, but... telling people to use the
Rust allocator where they can won't cause the problems PG.

[^1]: If we call a PG function which reports an `ERROR` (or anything else that
    gets it to longjmp), we convert it to a `panic` before immediately after it
    crosses into Rust, and if a panic unwinds all the way out of Rust, we
    convert it back to the appropriate postgres error, that is, it started as an
    error report caught by rust, then we remember and resore the error report
    information, making the longjmp->panic->longjmp round-trip lossless (in
    cases where nothing catches and manipulates the error in the middle).

    This is handled automatically by macros like `#[pg_extern]`, as well as all
    the wrappers which call into various `pg_sys` functions -- you don't have to
    do anything for it to work, even in cases where you manually call `pg_sys`
    functions.
