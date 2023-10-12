//LICENSE Portions Copyright 2019-2021 ZomboDB, LLC.
//LICENSE
//LICENSE Portions Copyright 2021-2023 Technology Concepts & Design, Inc.
//LICENSE
//LICENSE Portions Copyright 2023-2023 PgCentral Foundation, Inc. <contact@pgcentral.org>
//LICENSE
//LICENSE All rights reserved.
//LICENSE
//LICENSE Use of this source code is governed by the MIT license that can be found in the LICENSE file.
//! for converting primitive types into Datums
//!
//! Primitive types can never be null, so we do a direct
//! cast of the primitive type to pg_sys::Datum

use crate::{pg_sys, rust_regtypein, PgBox, PgOid, WhoAllocated};
use core::fmt::Display;
use pgrx_pg_sys::panic::ErrorReportable;
use pgrx_pg_sys::{Datum, Oid};
use std::any::Any;

/// Convert a Rust type into a `pg_sys::Datum`.
///
/// Default implementations are provided for the common Rust types.
///
/// If implementing this, also implement `FromDatum` for the reverse
/// conversion.
///
/// Note that any conversions that need to allocate memory (ie, for a `varlena *` representation
/// of a Rust type, that memory **must** be allocated within a [`PgMemoryContexts`](crate::PgMemoryContexts).
///
/// # Safety
/// This trait must be implemented correctly, or Very Bad Things will happen
/// (anything from data loss and DB corruption to memory unsafety and crashing).
///
/// A complete definition on what "correctly" means here is still a work in
/// progress, but at a minimum:
/// - [`IntoDatum`] must agree with [`FromDatum`], if both are implemented.
/// - [`IntoDatum`] and [`FromDatum`] must agree with anything Postgres knows or
///   believes about the type in question.
/// - Everything must behave as documented, including implicit documentation
///   like function and parameter names.
///
/// [`FromDatum`]: crate::FromDatum
pub unsafe trait IntoDatum {
    fn into_datum(self) -> Option<pg_sys::Datum>;
    fn type_oid() -> pg_sys::Oid;

    fn composite_type_oid(&self) -> Option<Oid> {
        None
    }
    fn array_type_oid() -> pg_sys::Oid {
        unsafe { pg_sys::get_array_type(Self::type_oid()) }
    }

    /// Is a Datum of this type compatible with another Postgres type?
    ///
    /// An example of this are the Postgres `text` and `varchar` types, which are both
    /// technically compatible from a Rust type perspective.  They're both represented in Rust as
    /// `String` (or `&str`), but the underlying Postgres types are different.
    ///
    /// If implementing this yourself, you likely want to follow a pattern like this:
    ///
    /// ```rust,no_run
    /// # use pgrx::*;
    /// # #[repr(transparent)]
    /// # struct FooType(String);
    /// # unsafe impl pgrx::IntoDatum for FooType {
    ///     fn is_compatible_with(other: pg_sys::Oid) -> bool {
    ///         // first, if our type is the other type, then we're compatible
    ///         Self::type_oid() == other
    ///
    ///         // and here's the other type we're compatible with
    ///         || other == pg_sys::VARCHAROID
    ///     }
    ///
    /// #    fn into_datum(self) -> Option<pg_sys::Datum> {
    /// #        todo!()
    /// #    }
    /// #
    /// #    fn type_oid() -> pg_sys::Oid {
    /// #        pg_sys::TEXTOID
    /// #    }
    /// # }
    /// ```
    #[inline]
    fn is_compatible_with(other: pg_sys::Oid) -> bool {
        Self::type_oid() == other
    }
}

/// for supporting NULL as the None value of an Option<T>
unsafe impl<T> IntoDatum for Option<T>
where
    T: IntoDatum,
{
    fn into_datum(self) -> Option<pg_sys::Datum> {
        match self {
            Some(t) => t.into_datum(),
            None => None,
        }
    }

    fn type_oid() -> pg_sys::Oid {
        T::type_oid()
    }
}

unsafe impl<T, E> IntoDatum for Result<T, E>
where
    T: IntoDatum,
    E: Any + Display,
{
    /// Returns The `Option<pg_sys::Datum>` representation of this Result's `Ok` variant.
    ///
    /// ## Panics
    ///
    /// If this Result represents an error, then that error is raised as a Postgres ERROR, using
    /// the [`PgSqlErrorCode::ERRCODE_DATA_EXCEPTION`] error code.
    ///
    /// If we detect that the `Err()` variant contains `[pg_sys::panic::ErrorReport]`, then we
    /// directly raise that as the error.  This enables users to set a specific "sql error code"
    /// for a returned error, along with providing the HINT and DETAIL lines of the error.
    #[inline]
    fn into_datum(self) -> Option<Datum> {
        self.report().into_datum()
    }

    #[inline]
    fn type_oid() -> pg_sys::Oid {
        T::type_oid()
    }
}

/// for bool
unsafe impl IntoDatum for bool {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(pg_sys::Datum::from(if self { 1 } else { 0 }))
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::BOOLOID
    }
}

/// for "char"
unsafe impl IntoDatum for i8 {
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(pg_sys::Datum::from(self))
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::CHAROID
    }
}

/// for smallint
unsafe impl IntoDatum for i16 {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(pg_sys::Datum::from(self))
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::INT2OID
    }

    fn is_compatible_with(other: pg_sys::Oid) -> bool {
        Self::type_oid() == other || i8::type_oid() == other
    }
}

/// for integer
unsafe impl IntoDatum for i32 {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(pg_sys::Datum::from(self))
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::INT4OID
    }

    fn is_compatible_with(other: pg_sys::Oid) -> bool {
        Self::type_oid() == other || i8::type_oid() == other || i16::type_oid() == other
    }
}

/// for oid
unsafe impl IntoDatum for u32 {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(pg_sys::Datum::from(self))
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::OIDOID
    }

    fn is_compatible_with(other: pg_sys::Oid) -> bool {
        Self::type_oid() == other
            || i8::type_oid() == other
            || i16::type_oid() == other
            || i32::type_oid() == other
    }
}

/// for bigint
unsafe impl IntoDatum for i64 {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(pg_sys::Datum::from(self))
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::INT8OID
    }

    fn is_compatible_with(other: pg_sys::Oid) -> bool {
        Self::type_oid() == other
            || i8::type_oid() == other
            || i16::type_oid() == other
            || i32::type_oid() == other
            || i64::type_oid() == other
    }
}

/// for real
unsafe impl IntoDatum for f32 {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(self.to_bits().into())
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::FLOAT4OID
    }
}

/// for double precision
unsafe impl IntoDatum for f64 {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(self.to_bits().into())
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::FLOAT8OID
    }
}

unsafe impl IntoDatum for pg_sys::Oid {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        if self == pg_sys::Oid::INVALID {
            None
        } else {
            Some(pg_sys::Datum::from(self.as_u32()))
        }
    }

    #[inline]
    fn type_oid() -> pg_sys::Oid {
        pg_sys::OIDOID
    }
}

unsafe impl IntoDatum for PgOid {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        match self {
            PgOid::Invalid => None,
            oid => Some(oid.value().into()),
        }
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::OIDOID
    }
}

/// for text, varchar
unsafe impl<'a> IntoDatum for &'a str {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        self.as_bytes().into_datum()
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::TEXTOID
    }

    #[inline]
    fn is_compatible_with(other: Oid) -> bool {
        Self::type_oid() == other || other == pg_sys::VARCHAROID
    }
}

unsafe impl IntoDatum for String {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        self.as_str().into_datum()
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::TEXTOID
    }

    #[inline]
    fn is_compatible_with(other: Oid) -> bool {
        Self::type_oid() == other || other == pg_sys::VARCHAROID
    }
}

unsafe impl IntoDatum for &String {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        self.as_str().into_datum()
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::TEXTOID
    }

    #[inline]
    fn is_compatible_with(other: Oid) -> bool {
        Self::type_oid() == other || other == pg_sys::VARCHAROID
    }
}

unsafe impl IntoDatum for char {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        self.to_string().into_datum()
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::VARCHAROID
    }

    #[inline]
    fn is_compatible_with(other: Oid) -> bool {
        Self::type_oid() == other || other == pg_sys::VARCHAROID
    }
}

/// for cstring
unsafe impl<'a> IntoDatum for &'a core::ffi::CStr {
    /// The [`core::ffi::CStr`] is copied to `palloc`'d memory.  That memory will either be freed by
    /// Postgres when [`pg_sys::CurrentMemoryContext`] is reset, or when the function you passed the
    /// returned Datum to decides to free it.
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        unsafe {
            // SAFETY:  A `CStr` has already been validated to be a non-null pointer to a null-terminated
            // "char *", and it won't ever overlap with a newly palloc'd block of memory.  Using
            // `to_bytes_with_nul()` ensures that we'll never try to palloc zero bytes -- it'll at
            // least always be 1 byte to hold the null terminator for the empty string.
            //
            // This is akin to Postgres' `pg_sys::pstrdup` or even `pg_sys::pnstrdup` functions, but
            // doing the copy ourselves allows us to elide the "strlen" or "strnlen" operations those
            // functions need to do; the byte slice returned from `to_bytes_with_nul` knows its length.
            let bytes = self.to_bytes_with_nul();
            let copy = pg_sys::palloc(bytes.len()).cast();
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), copy, bytes.len());
            Some(copy.into())
        }
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::CSTRINGOID
    }
}

unsafe impl IntoDatum for alloc::ffi::CString {
    /// The [`core::ffi::CString`] is copied to `palloc`'d memory.  That memory will either be freed by
    /// Postgres when [`pg_sys::CurrentMemoryContext`] is reset, or when the function you passed the
    /// returned Datum to decides to free it.
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        self.as_c_str().into_datum()
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::CSTRINGOID
    }
}

/// for bytea
unsafe impl<'a> IntoDatum for &'a [u8] {
    /// # Panics
    ///
    /// This function will panic if the string being converted to a datum is longer than a `u32`.
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        let len = pg_sys::VARHDRSZ + self.len();
        unsafe {
            // SAFETY:  palloc gives us a valid pointer and if there's not enough memory it'll raise an error
            let varlena = pg_sys::palloc(len) as *mut pg_sys::varlena;

            // SAFETY: `varlena` can properly cast into a `varattrib_4b` and all of what it contains is properly
            // allocated thanks to our call to `palloc` above
            let varattrib_4b = varlena
                .cast::<pg_sys::varattrib_4b>()
                .as_mut()
                .unwrap_unchecked()
                .va_4byte
                .as_mut();

            // This is the same as Postgres' `#define SET_VARSIZE_4B` (which have over in
            // `pgrx/src/varlena.rs`), however we're asserting that the input string isn't too big
            // for a Postgres varlena, since it's limited to 32bits -- in reality it's about half
            // that length, but this is good enough
            varattrib_4b.va_header = <usize as TryInto<u32>>::try_into(len)
                .expect("Rust string too large for a Postgres varlena datum")
                << 2u32;

            // SAFETY: src and dest pointers are valid, exactly `self.len()` bytes long,
            // and the `dest` was freshly allocated, thus non-overlapping
            std::ptr::copy_nonoverlapping(
                self.as_ptr().cast(),
                varattrib_4b.va_data.as_mut_ptr(),
                self.len(),
            );

            Some(Datum::from(varlena))
        }
    }

    #[inline]
    fn type_oid() -> pg_sys::Oid {
        pg_sys::BYTEAOID
    }
}

unsafe impl IntoDatum for Vec<u8> {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        (&self[..]).into_datum()
    }

    #[inline]
    fn type_oid() -> pg_sys::Oid {
        pg_sys::BYTEAOID
    }
}

/// for VOID
unsafe impl IntoDatum for () {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        // VOID isn't very useful, but Postgres represents it as a non-null Datum with a zero value
        Some(Datum::from(0))
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::VOIDOID
    }
}

/// for user types
unsafe impl<T, AllocatedBy: WhoAllocated> IntoDatum for PgBox<T, AllocatedBy> {
    #[inline]
    fn into_datum(self) -> Option<pg_sys::Datum> {
        if self.is_null() {
            None
        } else {
            Some(self.into_pg().into())
        }
    }

    fn type_oid() -> pg_sys::Oid {
        rust_regtypein::<T>()
    }
}

unsafe impl IntoDatum for pg_sys::Datum {
    fn into_datum(self) -> Option<pg_sys::Datum> {
        Some(self)
    }

    fn type_oid() -> pg_sys::Oid {
        pg_sys::INT8OID
    }
}
