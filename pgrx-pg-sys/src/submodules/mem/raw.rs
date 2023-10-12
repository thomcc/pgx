//! Low level, unsafe memory context operations.
//!
//! This exists to share code

use crate as pg_sys;
use core::{
    ffi::{c_void, CStr},
    marker::PhantomData,
    ptr::NonNull,
};
use pg_sys::{self, CurrentMemoryContext, MemoryContext};
use pg_sys::{MemoryContextData, NodeTag_T_AllocSetContext, TopMemoryContext};

/// This is a wrapper around a raw [`MemoryContext`]. It is returned by
/// `MemCtx::raw()`, and also used internally.
///
/// It is a more flexible alternative to `MemCtx` or `OwnedMemCtx`, and can be
/// used in cases where you'd otherwise turn to the
///
/// This offers some useful APIs, and offers slightly more guarantees than a raw
/// pointer, but because
///
#[repr(transparent)]
pub struct RawMemCtx {
    ptr: NonNull<MemoryContextData>,
}
#[inline(always)]
unsafe fn memory_context_is_valid(p: *mut MemoryContextData) -> bool {
    !p.is_null()
        && matches!(
            (*p).type_,
            pg_sys::NodeTag_T_AllocSetContext
                | pg_sys::NodeTag_T_SlabContext
                | pg_sys::NodeTag_T_GenerationContext
        )
}

impl RawMemCtx {
    ///
    #[inline]
    #[track_caller]
    pub unsafe fn from_raw(p: *mut MemoryContextData) -> Self {
        debug_assert!(memory_context_is_valid(p));
        Self { ptr: NonNull::new_unchecked(p) }
    }

    #[inline]
    pub unsafe fn alloc(&self, sz: usize) -> *mut c_void {
        pg_sys::palloc()
    }
}
