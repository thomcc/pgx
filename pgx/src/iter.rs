use pgx_utils::sql_entity_graph::metadata::{
    ArgumentError, Returns, ReturnsError, SqlMapping, SqlTranslatable,
};
use std::iter::once;

use crate::{pg_sys, IntoDatum};

pub struct SetOfIterator<T> {
    iter: Box<dyn Iterator<Item = T> + 'static>,
}

impl<T> SetOfIterator<T> {
    pub fn new<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: 'static,
    {
        Self { iter: Box::new(iter.into_iter()) }
    }
}

impl<T> Iterator for SetOfIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

unsafe impl<T> SqlTranslatable for SetOfIterator<T>
where
    T: SqlTranslatable,
{
    fn argument_sql() -> Result<SqlMapping, ArgumentError> {
        T::argument_sql()
    }
    fn return_sql() -> Result<Returns, ReturnsError> {
        match T::return_sql() {
            Ok(Returns::One(sql)) => Ok(Returns::SetOf(sql)),
            Ok(Returns::SetOf(_)) => Err(ReturnsError::NestedSetOf),
            Ok(Returns::Table(_)) => Err(ReturnsError::SetOfContainingTable),
            err @ Err(_) => err,
        }
    }
}

pub struct TableIterator<T> {
    iter: Box<dyn Iterator<Item = T> + 'static>,
}

impl<T> TableIterator<T> {
    pub fn new<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T> + 'static,
        I::IntoIter: 'static,
    {
        Self { iter: Box::new(iter.into_iter()) }
    }

    pub fn once(value: T) -> TableIterator<T>
    where
        T: 'static,
    {
        Self { iter: Box::new(once(value)) }
    }
}

impl<T: 'static> Iterator for TableIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<T> IntoDatum for TableIterator<T>
where
    T: SqlTranslatable,
{
    fn into_datum(self) -> Option<pg_sys::Datum> {
        todo!()
    }

    fn type_oid() -> pg_sys::Oid {
        todo!()
    }
}

seq_macro::seq!(I in 0..=32 {
    #(
        seq_macro::seq!(N in 0..=I {
            unsafe impl<#(Input~N,)*> SqlTranslatable for TableIterator<(#(Input~N,)*)>
            where
                #(
                    Input~N: SqlTranslatable + 'static,
                )*
            {
                fn argument_sql() -> Result<SqlMapping, ArgumentError> {
                    Err(ArgumentError::Table)
                }
                fn return_sql() -> Result<Returns, ReturnsError> {
                    let mut vec = Vec::new();
                    #(
                        vec.push(match Input~N::return_sql() {
                            Ok(Returns::One(sql)) => sql,
                            Ok(Returns::SetOf(_)) => return Err(ReturnsError::TableContainingSetOf),
                            Ok(Returns::Table(_)) => return Err(ReturnsError::NestedTable),
                            Err(err) => return Err(err),
                        });
                    )*
                    Ok(Returns::Table(vec))
                }
            }
        });
    )*
});
