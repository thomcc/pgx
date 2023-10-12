unrelated question. i have
```rs
/// for example:
///
/// - `OwnedAs<'mcx, &'mcx str, pg_sys::text>`
///
/// - `OwnedAs<'mcx, MoreComplexWrapper<'mcx>>`
struct OwnedAs<'mcx, T: 'mcx, RawPointee = c_void> {
    base: *mut RawPointee, // set to null if we shouldn't free it.
    // This is actually always initialized, but we need
    // MaybeUninit 
    derived: MaybeUninit<T>,
    _boo: PhantomData<&'mcx MemCx<'mcx>>,
}

impl<'mcx, T: 'mcx, RawPointee> Drop for Derived {
    fn drop(&mut self) {
        if !self.base.is_null() {
            unsafe {
                ManuallyDrop::drop(&mut self.derived);
                pg_sys::pfree(base);
            };
        }
    }
}
```