# What if PGRX, but safer? the `safer` branch docs

You might notice "This doesn't just fix SPI, this fixes a bunch of other things
too!". This is true. I tried several attempts to just fix SPI in various ways,
but the reality is that the constraint we have not only fixing SPI, but fixing
it in a way where future soundness fixes in PGRX would not lead to breaking
changes in the "fixed" SPI interface.

While 

1. First attempt just wrapping exiting SPI mostly, adding a couple traits on
   top. This might also be explained as "fix SPI by just tweaking datum traits"

2. Second attempt correctly realized that memory contexts needed fixing to fix
   SPI, but then decided "actually no they don't".

3. Third attempt ignored memory contexts, and just used SPI lifetimes. It was
   actually really tricky, but also would require future breakage if we ever
   fixed PGRX.

4. This attempt. In it, we decide that the best excuse is to just do the damn
   thing, and redesign how Postgres handles memory. That is: redesign memctx,
   propagate that to Datum, use that to fix SPI.

This doesn't change everything over to The New Paradigm, but we should do that.
However, the hope is that I don't need to carry it on my back along.

Some apologies in advance:

- This PR is big. Hopefully the fact that I wrote a lot of high level docs
  (since they helped me piece it all together) helps. These are the docs in this
  folder, the new module docs, and some of the type docs. I don't have super
  thorough function docs, aside from safety comments.

- Even worse, this PR is definitely bigger than it needs to be. Not every new
  piece is strictly needed.

    The reason they're here mostly has to do with the fact that it took so many
    attempts. Basically, if I made something better in an old attempt, I kept
    using it in later ones. Sorry.

- 

Some stuff that still needs to be done:

1. Deprecating and/or removing APIs which are made redundant by this change.
   `PgMemoryContexts` and `PgBox` are fully redundant, `FromDatum`/`IntoDatum`
   are a little bit.

2. Fully integrating new types/traits with the macro(s).

3. Using the new APIs in existing code that needs them.

4. 




## Behavioral Changes

The main one of these that doesn't have a corresponding 


The `safer` branch fixes a number of long-standing soundness holes in the core
of PGRX around memory management and datums. This folder contains documentation
about the changes, intended usage patterns, internals, plans, and so on. Most of
it is somewhere between user-facing or contributor-facing, closer to the latter.

I don't really know what to do with these docs, certainly we don't have stuff like them currently, but 




It is a significant change, although effort has been taken to reduce how much user-f



SPI, and more. It's a breaking change although 





This folder contains documentation for the changes in the `pgrx/safer` branch. It's a bit of a grab bag of stuff. I'm not sure it should , but an.

Some of it is written in a user-facing manner, some of it is internals
