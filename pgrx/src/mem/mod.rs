//! # PGRX Memory Management
//!
//! This module reconciles the (otherwise incompatible) memory management
//! strategies of Postgres and Rust. It allows safe code to manage memory in
//! certain ways, and allows unsafe code to build sound abstractions.
//!
//! It's a bit complex. The general advice is that most Rust code should use the
//! Rust allocator wherever it does not need to be using Postgres-managed
//! memory.
//!
//! ## Things that can't be expressed
//!
//! Some patterns common in PostgreSQL's C code are impossible to express in a
//! safe Rust API.
//!
//! A notable one is the pattern of a data structure holding a memory context
//! used to allocate its internal data structures (and in many cases, itself as
//! well). This is a self-referential borrow, which is something that Rust
//! cannot express with lifetimes, you must use raw pointers indefinitely.
//!
//!
//!
//! Postgres' memory management does not interact well with Rust's; the design
//! goals of both are fundamentally different.
//!
//! - Rust prioritizes memory safety over memory leaks, but Postgres' system is
//!   designed to prevent memory leaks even at the cost of making temporal
//!   memory safety violations (such as use-after-frees) somewhat more likely.
//!
//! - In Postgres it is not uncommon for types to hold a MemoryContext they use
//!   for their internal allocations and data structures. This is impossible to
//!   express in safe Rust, as it is a self-referential borrow. It is possible
//!   to express in fully unsafe code.
//!
//! `pgrx::mem` attempts to reconcile the two worlds, finally giving a path to
//! closing many long-standing soundness holes in PGRX's APIs. However, it comes
//! at the cost of some complexity and ergonomics, adding lifetimes to
//! essentially anything allocated from a memory context, and requiring
//! callbacks.
//!
//! `pgrx` has not fully moved over to using these APIs yet, but is expected
//! (hoped?) to over time.
//!
//!
//!
//! ## PGRX's solution
//!
// under non-pg16 we can't actually link to that function (it does not exist),
// so just link to the alignment section of the file instead.
#![cfg_attr(not(feature = "pg16"), doc("[`pg_sys::palloc_aligned`]: #alignment"))]
use pg_sys::MemoryContext;

use crate::pg_sys::{self, CurrentMemoryContext};
use core::{marker::PhantomData, ptr::NonNull};

pub mod ptrs;
pub mod raw;

/// A borrowed memory context.
pub struct MemCx<'mcx> {
    ptr: NonNull<MemoryContext>,
    _marker: PhantomData<&'mcx MemoryContext>,
}

// /// - `'datum` is the lifetime that impls may borrow from the input datum.
// /// - `'mcx` represents the borrow of the memory that impls may borrow from the
// ///   memory context.
// ///
// /// Types which borrow from either must represent those borrows in their type
// /// directly (via PhantomData,m)
// pub unsafe trait FromDatumSafe<'mcx, 'datum>: Sized {
//     unsafe fn from_datum_full(
//         datum: DatumRef<'datum>,
//         is_null: bool,
//         oid: Oid,
//         mcx: &'mcx MemCx<'mcx>,
//     ) -> Self;

//     unsafe fn from_datum(
//         datum: DatumRef<'datum>,
//         is_null: bool,
//         oid: Oid,
//         mcx: &'mcx MemCx<'mcx>,
//     ) -> Self;
// }
// pub unsafe trait IntoDatum<'this>: Sized {
//     unsafe fn raw_to_datum(&'this self, datum: DatumRef<'datum>, mcx: &'mcx MemCx<'mcx>) -> Self;
// }

// pub unsafe trait FromDatumOwned: for<'mcx, 'datum> FromDatumSafe<'mcx, 'datum> {}

// pub unsafe trait FromDatumSafe<'mcx, 'datum>: Sized {
//     unsafe fn from_datum(
//         datum: DatumRef<'datum>,
//         is_null: bool,
//         oid: Oid,
//         mcx: &'mcx MemCx<'mcx>,
//     ) -> Self;
// }
