// returns a string indicating which one if p is any of the global contexts.
unsafe fn identify_global(p: *mut MemoryContextData) -> Option<&'static str> {
    if p == pg_sys::TopMemoryContext {
        Some("TopMemoryContext")
    } else if p == pg_sys::ErrorContext {
        Some("ErrorContext")
    } else if p == pg_sys::PostmasterContext {
        Some("PostmasterContext")
    } else if p == pg_sys::CacheMemoryContext {
        Some("CacheMemoryContext")
    } else if p == pg_sys::MessageContext {
        Some("MessageContext")
    } else if p == pg_sys::TopTransactionContext {
        Some("TopTransactionContext")
    } else if p == pg_sys::CurTransactionContext {
        Some("CurTransactionContext")
    } else if p == pg_sys::PortalContext {
        Some("PortalContext")
    } else if p == pg_sys::CurrentMemoryContext {
        Some("CurrentMemoryContext")
    }
}

/// Returns a type that implements `Debug`.
///
/// This is provided rather than a `Debug` impl, because it must be an
/// unsafe function.
///
/// # Safety
///
/// This context must still be valid, and not deleted.
pub unsafe fn debug(&self) -> impl core::fmt::Debug + 'a {
    struct Wrapper<'a>(&'a RawMemCtx);
    impl core::fmt::Debug for Wrapper<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            // Safety: the function that returned the Wrapper is
            // unsafe, and the caller asserts these requirements.
            unsafe {
                let p = self.0.ptr.as_ptr();
                debug_assert!(memory_context_is_valid(p));
                if f.alternate() {
                    // handle {:#?} with nodeToString.
                    let ptr = pg_sys::nodeToString(p);
                    // use `pad` to respect width flags (just because we have the whole string)
                    let res = f.pad(&CStr::from_ptr(ptr).to_string_lossy());
                    pg_sys::pfree(ptr.cast());
                    return res;
                } else {
                }
                let fmtstr = |p: *const core::ffi::c_char| -> &'static CStr {
                    if p.is_null() {
                        CStr::from_bytes_with_nul(b"(null)\0").unwrap()
                    } else {
                        CStr::from_ptr(p)
                    }
                };
                let global = identify_global(p);
                let mut builder = f.debug_struct("RawMemCtx").field("name", &fmtstr((*p).name));
            }
        }
    }
}
#[warn(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn fmt_node(node: NonNull<crate::Node>, f: &mut core::fmt::Formatter) {
    // SAFETY: It's fine to call nodeToString with non-null well-typed pointers,
    // and pg_sys::nodeToString() returns data via palloc, which is never null
    // as Postgres will ERROR rather than giving us a null pointer,
    // and Postgres starts and finishes constructing StringInfos by writing '\0'
    let node_cstr = DeferPfree(crate::nodeToString(node.as_ptr().cast()));
    let res = f.pad(&CStr::from_ptr(node_cstr).to_string_lossy());
    drop(node_cstr);
    res
}

struct DeferPfree(*mut core::ffi::c_void);
impl Drop for DeferPfree {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { crate::pfree(self.0) };
        }
    }
}
