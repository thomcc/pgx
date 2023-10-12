# TODO

## Follow-ups

### Naming
Rename `Datum` to `RawDatum` and `DatumLt<'a>` to `Datum<'a>`.

In general maybe the names I've used are bad. I perhaps am from the generation
of Rust programmers who overuse abbreviations. `MemCx` gets used everywhere so
`BorrowedMemoryContext` would be a nightmare, but perhaps `MemCx` is a step too
far.

### Integration into macros

Accepting a memory context in a `pg_extern` function should be possible, but
currently is not.
```rs
#[pg_extern]
fn foobar<'cur>(cx: &'cur MemCx<'cur>, x: i32, y: &str) -> SomePgrxType<'cur>;
```
This argument should be that we ignored for generating SQL, and at runtime we'd
implement it by borrowing the CurrentMemoryContext before the call.

## Deprecations
All the following need to be removed / deprecated
- `PgMemoryContext`.
- `PgBox`.
- The whole concept of "who allocated" memory being relevant -- you care about two things:
  - It's lifetime.
  - Who *frees* it.
- 


## General notes around PL/Rust
- Let's expose this in as limited a way as we can.


## Unrelated to this work, but noticed during it

- OIDs need an overhall
  - need a `TypeOid`
  - `u32` -> `Oid` should be safe (`select foo::oid`)
  - may

- `regtypein` may look up in wrong schema.
- 
