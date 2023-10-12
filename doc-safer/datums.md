# `Datum`

## Datum Lifetimes

My initial belief was that solving memory contexts would allow us to avoid ever
needing giving a liftime to a datum (without also giving a type to the datum).

This was wrong. Everybody who said datums need a life time was right -- and
obviously so now that I fully understand the relevant pieces.

It is still necessary to assign a lifetime to datums as datums may contain a
pointer. While on their own, datums do not carry enough information to *access*
the memory (you need separate type information, such as an OID), to do so.

We have `DatumLt<'datum>`, which is just a datum with a lifetime (despite the
name, it is not a reference to a datum). The `'datum` lifetime is the lifetime
which may be used for any memory memory extracted from the datum.






The lifetime is
the lifetime that memory 


The name is a little bad, as it's not a reference to a
datum, or a datum with a lifetime.


 (initially it was
`DatumLt<'a>`, but this looked too much like "reference to a datum" to me).
Bikeshed painting is possible here.



1. Types which are represented as a Datum directly . These
   are all `typbyval` types



