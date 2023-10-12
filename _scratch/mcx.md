
/// # Lifetimes
///
/// You may be wondering about `&'cx MemCx<'cx>`, which is a reference, but also
/// has an inner lifetime. Is that really needed? Does the distinction between
/// these matter? The answers are "for the most part, yes", and "sort of, the
/// lifetime on the reference the `&'cx` one is the one that matters. You can
/// always either set the inner one to the same (`&'mcx MemCx<'mcx>`) or elide
/// it (`&'mcx MemCx<'_>`) and be fine.
///
/// If you want to define a trait that does takes a context and allocates with
/// it, there are various concerns areound the things that implement the trait.
///
/// In general, the fully flexible way is something like this:
/// ```ignore
/// trait NewThing<'mcx>: Sized {
///     fn new_thing(cx: &'cx MemCx<'cx>) -> Self;
/// }
/// // also
/// trait AppendeeShapedThing<'mcx>: Sized {
///     fn add<'me: 'mcx>(&'me mut self, cx: &'mcx MemCx<'mcx>, stuff: Stuff);
/// }
/// ```
///
/// Note that the `'sized
///
/// There are important caveats here around here for PL/Rust, to ensure we don't
/// circumvent our own soundness enforcement workarounds. To  If possible, make
/// the trait not be object safe.
///
/// Just here to know what you should use?
///
/// - Putting it in a struct, enum or other type -- IOW, is naming both
///   lifetimes is required? `&'cx MemCx<'cx>` is fine for all but the weirdest
///   cases.
/// - P
///
/// If so: use `&'cx MemCx<'cx>` (possibly behind phantomdata). This is also
/// valid for any of these with the exption of tra
///
/// ```ignore
/// /// Fully general zero-copy access to potentially toasted-required
/// /// requires something like this, because untoasted data is borrowed
/// /// from the datum directly, e.g. `Datum<'datum>`, whereas toasted data
/// /// must be allocated in the memory context.
/// unsafe impl trait RawFromDatum<'mcx, 'datum>: Sized {
///     unsafe fn from_datum(
///         datum: DatumRef<'datum>,
///         is_null: bool,
///         oid: Oid,
///         mcx: &'mcx MemCx<'mcx>,
///      ) -> Self;
/// }
/// // However
/// unsafe impl<'me> trait RawFromDatum<'mcx> {
///     fn from_datum<'datum>(mcx: &'mcx MemCx<'_>, datum: RefDatum<'datum>) -> Self;
/// }
/// ```
///
/// - Is it a function parameter? You probably want `&'cx MemCx<'_>`
///
/// - Almost certanly `&'cx MemCx<'_>` or `&'cx MemCx<'cx>`. It's almost always
///   fine to just try something and see what compiles, any safe code that
///   misuses this API should get a compile error.
///
///
///
/// Borrowing a context almost always hands out a `&'cx MemCx<'cx>`. However, if
/// this were *truly* guaranteed to be the same, then we would presumably not
/// need both (probably?).
///
/// The truth is there are a few rare cases where they are not the same.
/// Cocnretely, they are `&'this_cx_borrow MemCx<'other_cx_borrow>`.
///
/// The `'outer_context_borrow` ex
///
/// lifetime parameter is almost never important, user code can feel free to
/// ignore it, and use `&'cx MemCx<'_>`.
///
/// If you need to give a name to both lifetimes, using the same as the
/// reference, as in `&'cx MemCx<'cx>` is fine. The lifetime parameter
///
/// # Usage
/// Currently, `MemCx` always handed out via a reference. For example, `&'mcx
///
/// ## Details
///