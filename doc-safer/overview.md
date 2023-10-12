# Overview and Conceptual Changes

Actually redesigning memory management in a way we won't regret required 

- Try to use the new RAII stuff to guard against leaking on error. See
- 

## Big ideas

### New memory context types

- `&MemCtx<'_>` is a borrowed memory context.

    # The Two Lifetimes

    You may note that there are two lifetimes. If it helps to think of them as
    `&'self_borrow MemCtx<'parent_borrow>`, then do so. Note, however, that
    `'parent_borrow` is rarely of the parent context.

    The reason it exists is to allow enforcing the `'parent_brow: 'self_brow`
    relation inside SPI.

    know what to do with them: when in doubt `&'cx MemCtx<'cx>` (making them the
    same) is fine. You can also only name the outer one `&'cx MemCx<'_>`, as it
    is the one which is used for allocations.

    The inner borrow is irrelevant for everything besides SPI currently.
    Concretely, the two are related as so:
    - `'inner` represents a borrow of some 

    Technically, there are two lifetimes here `&'outer MemCx<'inner>`. Currently
    SPI is the only thing where they are not the same, and may stay this way.
    The relationship is that `'inner: 'outer`, i.e. inner outlives outer.

    The `'inner` lifetime represents some lifetime which outlives the borrow of
    the context itself. An example might be that the lifetime of the parent
    context, although we don't use it for that anywhere currently.

    In SPI, `'inner` is the borrow of the `CurrentMemoryContext` prior to
    connecting to SPI. If you are thinking "SPI's context are children of
    TopMemoryContext so they may 


     can be used to represent the "return context
    memory outside of a given SPI scope -- perhaps the `CurrentMemoryContext`
    from before the SPI was opened.

    That said, SPI is a unique case -- 99% of the code has no reason to
    represent such things allocations where 

    Safe code can probaly ignore the lifetimes except when they cause errors.

    The strict understanding of both lifetimes is `&'borrow MemCx<'outer>`,
    where `'borrow` is the duration of the borrow of this context, and the
    `'outer` lifetime is the duration of the borrow of the paren

    where `'outer` is either the lifetime. 

    This distiction is not terribly important most of the time, since almost all
    borrows are not from directly-scoped lifetimes, and so we


    If
    you need to name them, `&'cx MemCx<'cx>` is ideally the full, but if you can
    way, but if you only need to name one of them that's fine too. There is not
    really a way to do this that can lead to UB in safe code.

    The inner lifetime represents the parent lifetime if any. It outlives the outer life


    , `&'cx MemCx<'cx>`. We can relax
    this in the future if needed.

    If you are asking: Why both represent this as two lifetimes? The answer is:
    outer lifetime doesn't really matter, it just enforces what kind of borrow
    this is.

    For example `&'cx MemCx<'cx>` and `&'cx mut MemCx<'cx>` could have different
    APIs. 


    Memory allocated from it should be marked with `'cx`. A `PhantomData<&'cx
    MemCx<'cx>>` is very precise but you could even do `&'cx ()` or whatever.
    See `PBox<'cx, T>` for an example.

    As for how to handle them in code, there are two options:
    - Code that doesn't need to name the lifetimes doesn't have to.
    - If you need to name both of them, for example, defining a structure, use
      thse same for both, e.g. `&'mcx MemCtx<'mcx>`, which will do the right
      thing.
    - If you need to name them in a trait, this is trickier -- you should

```rs
trait Foo<'cx> {
    fn creatify(m: &'cx MemCx<'cx>) -> 
}
```

     In practice, you can either
    make these the same () or ignore the outer one. The
    technical reason for this is twofold
    - The borrow of the context is shorter than the borrow of the data. We want
      



  While a memory context
  is borrowed, it may not be reset. The lifetime is covariant, so we probably
  avoid the worst lifetime pain. We hand these out with a closure, e.g. `blah.borrow(|ctx: &MemCtx| {})`
- `MemCtxRaw` wraps a raw pointer to a memory context. It exists mostly to 

- ~~`MemCtxOwned` and `MemCtxChild<'parent>` have been removed from the PR.~~

Memory contexts can be borrowed for a Rust scope. In the case of the current
memory context (at a minimum), the scope is represented as a closure, for
various reasons.

Note that the argument passed into the context

```rs
current::borrow(|ctx: &MemCtxRef<'cx>| {
    ctx.
});
```



## New Internal Stuff

### WithDrop

This change makes users are far more likely to use memory contexts in various
ways. At the moment the ways are relatively restricted, 

This means that it's more likely that something we leak will be a real long-term
leak.

One goal is for us to be better about not leaking, including for
`panic`/`ereport(ERROR)`. This means being slightly better about using RAII
types.

Doing this robustly and universally is impractical, we need it too often. 

```rs



```

### C string stuff

There's a whole thing for c-strings. To some extent it is intended to replace
`AsPgCStr`, which can be misused pretty easily.

This doesn't need to be part of this PR, but I wrote it in the past, decided it
was error-prone, and now was able to make it not error-prone.

```

```
