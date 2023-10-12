//! Misc internal utilities. Stuff like shims for missing language features,
//! perhaps.
//!
//! Nothing in here should be publically exposed, just move it to `pgrx::misc`
//! if we want to do that.

/// This is an annoying one. Imagine code that wants to represent a pointer it
/// must `pfree`, and then also represent
///
/// ```ignore (exposition only)
/// pub struct Owned<'mcx, T: 'mcx> {
///     ptr_for_pfree: *mut c_void,
///     derived: ManuallyDrop<T>,
///     _pd: PhantomData<&'mcx MemCx<'mcx>>,
/// }
/// impl<'mcx, T: 'mcx> Drop for Owned<'mcx, T> {
///     fn drop(&mut self) {
///         // Clean up anything `derived` needs
///         unsafe { ManuallyDrop::drop(&mut derived) };
///         // Free the pointer
///         unsafe { pg_sys::pfree(self.pointer_for_pfree) };
///     }
/// }
/// ```
/// This might be used for something like `Owned<'mcx, &[u8]>`, but also
/// `Owned<'mcx,
///
pub(crate) struct MaybeDangling<T> {}
