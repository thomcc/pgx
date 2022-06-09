/*
Portions Copyright 2019-2021 ZomboDB, LLC.
Portions Copyright 2021-2022 Technology Concepts & Design, Inc. <support@tcdi.com>

All rights reserved.

Use of this source code is governed by the MIT license that can be found in the LICENSE file.
*/
use std::ops::Deref;

use crate::anonymonize_lifetimes;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    FnArg, Pat, Token,
};

use super::entity::CompositeTypeWrapper;

/// A parsed `#[pg_extern]` argument.
///
/// It is created during [`PgExtern`](crate::sql_entity_graph::PgExtern) parsing.
#[derive(Debug, Clone)]
pub struct PgExternArgument {
    pat: syn::Ident,
    ty: syn::Type,
    /// Set via `composite_type!()`
    sql: Option<(syn::Expr, CompositeTypeWrapper)>,
    /// Set via `default!()`
    default: Option<String>,
    /// Set via `variadic!()`
    variadic: bool,
    optional: bool,
}

impl PgExternArgument {
    pub fn build(value: FnArg) -> Result<Option<Self>, syn::Error> {
        match value {
            syn::FnArg::Typed(pat) => Self::build_from_pat_type(pat),
            _ => Err(syn::Error::new(Span::call_site(), "Unable to parse FnArg")),
        }
    }

    pub fn build_from_pat_type(value: syn::PatType) -> Result<Option<Self>, syn::Error> {
        let mut true_ty = *value.ty.clone();
        anonymonize_lifetimes(&mut true_ty);

        let identifier = match *value.pat {
            Pat::Ident(ref p) => p.ident.clone(),
            Pat::Reference(ref p_ref) => match *p_ref.pat {
                Pat::Ident(ref inner_ident) => inner_ident.ident.clone(),
                _ => return Err(syn::Error::new(Span::call_site(), "Unable to parse FnArg")),
            },
            _ => return Err(syn::Error::new(Span::call_site(), "Unable to parse FnArg")),
        };

        let (mut true_ty, optional, variadic, default, sql) = resolve_arg_ty(*value.ty)?;

        // We special case ignore `*mut pg_sys::FunctionCallInfoData`
        match true_ty {
            syn::Type::Reference(ref mut ty_ref) => {
                if let Some(ref mut lifetime) = &mut ty_ref.lifetime {
                    lifetime.ident = syn::Ident::new("static", Span::call_site());
                }
            }
            syn::Type::Path(ref mut path) => {
                let segments = &mut path.path;
                let mut saw_pg_sys = false;
                let mut saw_functioncallinfobasedata = false;

                for segment in &mut segments.segments {
                    let ident_string = segment.ident.to_string();
                    match ident_string.as_str() {
                        "pg_sys" => saw_pg_sys = true,
                        "FunctionCallInfo" => saw_functioncallinfobasedata = true,
                        _ => (),
                    }
                }
                if (saw_pg_sys && saw_functioncallinfobasedata)
                    || (saw_functioncallinfobasedata && segments.segments.len() == 1)
                {
                    return Ok(None);
                } else {
                    for segment in &mut path.path.segments {
                        match &mut segment.arguments {
                            syn::PathArguments::AngleBracketed(ref mut inside_brackets) => {
                                for mut arg in &mut inside_brackets.args {
                                    match &mut arg {
                                        syn::GenericArgument::Lifetime(ref mut lifetime) => {
                                            lifetime.ident =
                                                syn::Ident::new("static", Span::call_site())
                                        }
                                        _ => (),
                                    }
                                }
                            }
                            _ => (),
                        }
                    }
                }
            }
            syn::Type::Ptr(ref ptr) => match *ptr.elem {
                syn::Type::Path(ref path) => {
                    let segments = &path.path;
                    let mut saw_pg_sys = false;
                    let mut saw_functioncallinfobasedata = false;
                    for segment in &segments.segments {
                        if segment.ident.to_string() == "pg_sys" {
                            saw_pg_sys = true;
                        }
                        if segment.ident.to_string() == "FunctionCallInfo" {
                            saw_functioncallinfobasedata = true;
                        }
                    }
                    if (saw_pg_sys && saw_functioncallinfobasedata)
                        || (saw_functioncallinfobasedata && segments.segments.len() == 1)
                    {
                        // It's a FunctionCallInfoBaseData, skipping
                        return Ok(None);
                    }
                }
                _ => (),
            },
            _ => (),
        };

        Ok(Some(PgExternArgument {
            pat: identifier,
            ty: true_ty,
            sql,
            default,
            variadic,
            optional,
        }))
    }
}

fn handle_composite_type_macro(mac: &syn::Macro) -> syn::Result<syn::Expr> {
    let out: syn::Expr = mac.parse_body()?;
    Ok(out)
}

fn handle_default_macro(mac: &syn::Macro) -> syn::Result<(syn::Type, Option<String>)> {
    let out: DefaultMacro = mac.parse_body()?;
    let true_ty = out.ty;
    match out.expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(def),
            ..
        }) => {
            let value = def.value();
            Ok((true_ty, Some(value)))
        }
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Float(def),
            ..
        }) => {
            let value = def.base10_digits();
            Ok((true_ty, Some(value.to_string())))
        }
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(def),
            ..
        }) => {
            let value = def.base10_digits();
            Ok((true_ty, Some(value.to_string())))
        }
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Bool(def),
            ..
        }) => {
            let value = def.value();
            Ok((true_ty, Some(value.to_string())))
        }
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            ref expr,
            ..
        }) => match &**expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(def),
                ..
            }) => {
                let value = def.base10_digits();
                Ok((true_ty, Some("-".to_owned() + value)))
            }
            _ => {
                return Err(syn::Error::new(
                    Span::call_site(),
                    format!(
                        "Unrecognized UnaryExpr in `default!()` macro, got: {:?}",
                        out.expr
                    ),
                ))
            }
        },
        syn::Expr::Type(syn::ExprType { ref ty, .. }) => match ty.deref() {
            syn::Type::Path(syn::TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => {
                let last = segments.last().expect("No last segment");
                let last_string = last.ident.to_string();
                if last_string.as_str() == "NULL" {
                    Ok((true_ty, Some(last_string)))
                } else {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        format!(
                            "Unable to parse default value of `default!()` macro, got: {:?}",
                            out.expr
                        ),
                    ));
                }
            }
            _ => {
                return Err(syn::Error::new(
                    Span::call_site(),
                    format!(
                        "Unable to parse default value of `default!()` macro, got: {:?}",
                        out.expr
                    ),
                ))
            }
        },
        syn::Expr::Path(syn::ExprPath {
            path: syn::Path { ref segments, .. },
            ..
        }) => {
            let last = segments.last().expect("No last segment");
            let last_string = last.ident.to_string();
            if last_string.as_str() == "NULL" {
                Ok((true_ty, Some(last_string)))
            } else {
                return Err(syn::Error::new(
                    Span::call_site(),
                    format!(
                        "Unable to parse default value of `default!()` macro, got: {:?}",
                        out.expr
                    ),
                ));
            }
        }
        _ => {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "Unable to parse default value of `default!()` macro, got: {:?}",
                    out.expr
                ),
            ))
        }
    }
}

impl ToTokens for PgExternArgument {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let is_optional = self.optional;
        let is_variadic = self.variadic;
        let pat = &self.pat;
        let default = self.default.iter();
        let mut ty = self.ty.clone();
        anonymonize_lifetimes(&mut ty);

        let ty_string = ty.to_token_stream().to_string().replace(" ", "");

        let ty_entity = match &self.sql {
            Some((sql, wrapper)) => {
                quote! {
                    ::pgx::utils::sql_entity_graph::TypeEntity::CompositeType {
                        sql: #sql,
                        wrapper: #wrapper,
                    }
                }
            }
            None => {
                quote! {
                    ::pgx::utils::sql_entity_graph::TypeEntity::Type {
                        ty_source: #ty_string,
                        ty_id: TypeId::of::<#ty>(),
                        full_path: core::any::type_name::<#ty>(),
                        module_path: {
                            let ty_name = core::any::type_name::<#ty>();
                            let mut path_items: Vec<_> = ty_name.split("::").collect();
                            let _ = path_items.pop(); // Drop the one we don't want.
                            path_items.join("::")
                        },
                    }
                }
            }
        };

        let quoted = quote! {
            ::pgx::utils::sql_entity_graph::PgExternArgumentEntity {
                pattern: stringify!(#pat),
                ty: #ty_entity,
                is_optional: #is_optional,
                is_variadic: #is_variadic,
                default: None #( .unwrap_or(Some(#default)) )*,
            }
        };
        tokens.append_all(quoted);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DefaultMacro {
    ty: syn::Type,
    pub(crate) expr: syn::Expr,
}

impl Parse for DefaultMacro {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let ty = input.parse()?;
        let _comma: Token![,] = input.parse()?;
        let expr = input.parse()?;
        Ok(Self { ty, expr })
    }
}

/** Resolves a `pg_extern` argument `syn::Type` into metadata

Returns `(resolved_ty, optional, variadic, default, sql)`.

It resolves the following macros:

* `pgx::default!()`
* `pgx::composite_type!()`
*/
fn resolve_arg_ty(
    ty: syn::Type,
) -> syn::Result<(
    syn::Type,
    bool,
    bool,
    Option<String>,
    Option<(syn::Expr, CompositeTypeWrapper)>,
)> {
    // There are three steps:
    // * Resolve the `default!()` macro
    // * Resolve `composite_type!()`
    // * Resolving any flags for that resolved type so we can not have to do this later.

    // Resolve any `default` macro
    // We do this first as it's **always** in the first position. It's not valid deeper in the type.
    let (ty, default) = match ty.clone() {
        // default!(..)
        // composite_type!(..)
        syn::Type::Macro(macro_pat) => {
            let mac = &macro_pat.mac;
            let archetype = mac.path.segments.last().expect("No last segment");
            match archetype.ident.to_string().as_str() {
                "default" => {
                    let (maybe_resolved_ty, default) = handle_default_macro(mac)?;
                    (maybe_resolved_ty, default)
                }
                _ => (syn::Type::Macro(macro_pat), None),
            }
        }
        original => (original, None),
    };

    // Now, resolve any `composite_type` macro
    let (ty, sql) = match ty {
        // composite_type!(..)
        syn::Type::Macro(macro_pat) => {
            let mac = &macro_pat.mac;
            let archetype = mac.path.segments.last().expect("No last segment");
            match archetype.ident.to_string().as_str() {
                "default" => {
                    return Err(syn::Error::new(
                        mac.span(),
                        "default!(default!()) not supported, use it only once",
                    ))?
                }
                "composite_type" => {
                    let sql = Some((
                        handle_composite_type_macro(&mac)?,
                        CompositeTypeWrapper::None,
                    ));
                    let ty = syn::parse_quote! {
                        ::pgx::PgHeapTuple<'_, ::pgx::AllocatedByRust>
                    };
                    (ty, sql)
                }
                _ => (syn::Type::Macro(macro_pat), None),
            }
        }
        syn::Type::Path(path) => {
            let segments = path.path.clone();
            let last = segments.segments.last().ok_or(syn::Error::new(
                path.span(),
                "Could not read last segment of path",
            ))?;

            match last.ident.to_string().as_str() {
                // Option<composite_type!(..)>
                // Option<Vec<composite_type!(..)>>
                // Option<Vec<Option<composite_type!(..)>>>
                // Option<VariadicArray<composite_type!(..)>>
                // Option<VariadicArray<Option<composite_type!(..)>>>
                "Option" => resolve_option_inner(
                    path,
                    last.arguments.clone(),
                    CompositeTypeWrapper::Option,
                )?,
                // Vec<composite_type!(..)>
                // Vec<Option<composite_type!(..)>>
                "Vec" => {
                    resolve_vec_inner(path, last.arguments.clone(), CompositeTypeWrapper::Vec)?
                }
                // VariadicArray<composite_type!(..)>
                // VariadicArray<Option<composite_type!(..)>>
                "VariadicArray" => resolve_variadic_array_inner(
                    path,
                    last.arguments.clone(),
                    CompositeTypeWrapper::VariadicArray,
                )?,
                // Array<composite_type!(..)>
                // Array<Option<composite_type!(..)>>
                "Array" => resolve_array_inner(
                    path,
                    last.arguments.clone(),
                    CompositeTypeWrapper::Array,
                )?,
                _ => (syn::Type::Path(path), None),
            }
        }
        original => (original, None),
    };

    // In this second setp, we go look at the resolved type and determine if it is a variadic, optional, etc.
    let (ty, variadic, optional) = match ty {
        syn::Type::Path(type_path) => {
            let path = &type_path.path;
            let last_segment = path.segments.last().ok_or(syn::Error::new(
                path.span(),
                "No last segment found while scanning path",
            ))?;
            let ident_string = last_segment.ident.to_string();
            match ident_string.as_str() {
                "Option" => {
                    // Option<VariadicArray<T>>
                    match &last_segment.arguments {
                        syn::PathArguments::AngleBracketed(angle_bracketed) => {
                            match angle_bracketed.args.first().ok_or(syn::Error::new(
                                angle_bracketed.span(),
                                "No inner arg for Option<T> found",
                            ))? {
                                syn::GenericArgument::Type(ty) => {
                                    match ty {
                                        // Option<VariadicArray<T>>
                                        syn::Type::Path(ref inner_type_path) => {
                                            let path = &inner_type_path.path;
                                            let last_segment =
                                                path.segments.last().ok_or(syn::Error::new(
                                                    path.span(),
                                                    "No last segment found while scanning path",
                                                ))?;
                                            let ident_string = last_segment.ident.to_string();
                                            match ident_string.as_str() {
                                                // Option<VariadicArray<T>>
                                                "VariadicArray" => {
                                                    (syn::Type::Path(type_path), true, true)
                                                }
                                                _ => (syn::Type::Path(type_path), false, true),
                                            }
                                        }
                                        // Option<T>
                                        _ => (syn::Type::Path(type_path), false, true),
                                    }
                                }
                                // Option<T>
                                _ => (syn::Type::Path(type_path), false, true),
                            }
                        }
                        // Option<T>
                        _ => (syn::Type::Path(type_path), false, true),
                    }
                }
                // VariadicArray<T>
                "VariadicArray" => (syn::Type::Path(type_path), true, false),
                // T
                _ => (syn::Type::Path(type_path), false, false),
            }
        }
        original => (original, false, false),
    };

    Ok((ty, optional, variadic, default, sql))
}

fn resolve_vec_inner(
    original: syn::TypePath,
    arguments: syn::PathArguments,
    wrapper_so_far: CompositeTypeWrapper,
) -> syn::Result<(syn::Type, Option<(syn::Expr, CompositeTypeWrapper)>)> {
    match arguments {
        syn::PathArguments::AngleBracketed(path_arg) => match path_arg.args.first() {
            Some(syn::GenericArgument::Type(ty)) => match ty.clone() {
                syn::Type::Macro(macro_pat) => {
                    let mac = &macro_pat.mac;
                    let archetype = mac.path.segments.last().expect("No last segment");
                    match archetype.ident.to_string().as_str() {
                        "default" => {
                            return Err(syn::Error::new(mac.span(), "`Vec<default!(T, default)>` not supported, choose `default!(Vec<T>, ident)` instead"))?;
                        }
                        "composite_type" => {
                            let sql = Some(handle_composite_type_macro(mac)?);
                            let ty = syn::parse_quote! {
                                Vec<::pgx::PgHeapTuple<'_, ::pgx::AllocatedByRust>>
                            };
                            Ok((ty, sql.map(|v| (v, wrapper_so_far))))
                        }
                        _ => Ok((syn::Type::Path(original), None)),
                    }
                }
                syn::Type::Path(arg_type_path) => {
                    let last = arg_type_path.path.segments.last().ok_or(syn::Error::new(
                        arg_type_path.span(),
                        "No last segment in type path",
                    ))?;
                    match last.ident.to_string().as_str() {
                        "Option" => {
                            let wrapper = match wrapper_so_far {
                                    CompositeTypeWrapper::Vec => CompositeTypeWrapper::VecOption,
                                    CompositeTypeWrapper::OptionVec => CompositeTypeWrapper::OptionVecOption,
                                    _ => return Err(syn::Error::new(last.span(), "Only Vec<..>, Option<Vec<..>>, Option<Vec<Option<..>> are valid"))?,
                                };
                            resolve_option_inner(original, last.arguments.clone(), wrapper)
                        }
                        _ => Ok((syn::Type::Path(original), None)),
                    }
                }
                _ => Ok((syn::Type::Path(original), None)),
            },
            _ => Ok((syn::Type::Path(original), None)),
        },
        _ => Ok((syn::Type::Path(original), None)),
    }
}

fn resolve_variadic_array_inner(
    original: syn::TypePath,
    arguments: syn::PathArguments,
    wrapper_so_far: CompositeTypeWrapper,
) -> syn::Result<(syn::Type, Option<(syn::Expr, CompositeTypeWrapper)>)> {
    match arguments {
        syn::PathArguments::AngleBracketed(path_arg) => match path_arg.args.first() {
            Some(syn::GenericArgument::Type(ty)) => match ty.clone() {
                syn::Type::Macro(macro_pat) => {
                    let mac = &macro_pat.mac;
                    let archetype = mac.path.segments.last().expect("No last segment");
                    match archetype.ident.to_string().as_str() {
                        "default" => {
                            return Err(syn::Error::new(mac.span(), "`VariadicArray<default!(T, default)>` not supported, choose `default!(VariadicArray<T>, ident)` instead"))?;
                        }
                        "composite_type" => {
                            let sql = Some(handle_composite_type_macro(mac)?);
                            let ty = syn::parse_quote! {
                                ::pgx::VariadicArray<::pgx::PgHeapTuple<'_, ::pgx::AllocatedByRust>>
                            };
                            Ok((ty, sql.map(|v| (v, wrapper_so_far))))
                        }
                        _ => Ok((syn::Type::Path(original), None)),
                    }
                }
                syn::Type::Path(arg_type_path) => {
                    let last = arg_type_path.path.segments.last().ok_or(syn::Error::new(
                        arg_type_path.span(),
                        "No last segment in type path",
                    ))?;
                    match last.ident.to_string().as_str() {
                        "Option" => {
                            let wrapper = match wrapper_so_far {
                                    CompositeTypeWrapper::OptionVariadicArray => CompositeTypeWrapper::OptionVariadicArrayOption,
                                    CompositeTypeWrapper::VariadicArray => CompositeTypeWrapper::VariadicArrayOption,
                                    _ => return Err(syn::Error::new(last.span(), "Only VariadicArray<..>, Option<VariadicArray<..>, and Option<VariadicArray<Option<..>> are valid"))?,
                                };
                            resolve_option_inner(original, last.arguments.clone(), wrapper)
                        }
                        _ => Ok((syn::Type::Path(original), None)),
                    }
                }
                _ => Ok((syn::Type::Path(original), None)),
            },
            _ => Ok((syn::Type::Path(original), None)),
        },
        _ => Ok((syn::Type::Path(original), None)),
    }
}

fn resolve_array_inner(
    original: syn::TypePath,
    arguments: syn::PathArguments,
    wrapper_so_far: CompositeTypeWrapper,
) -> syn::Result<(syn::Type, Option<(syn::Expr, CompositeTypeWrapper)>)> {
    match arguments {
        syn::PathArguments::AngleBracketed(path_arg) => match path_arg.args.first() {
            Some(syn::GenericArgument::Type(ty)) => match ty.clone() {
                syn::Type::Macro(macro_pat) => {
                    let mac = &macro_pat.mac;
                    let archetype = mac.path.segments.last().expect("No last segment");
                    match archetype.ident.to_string().as_str() {
                        "default" => {
                            return Err(syn::Error::new(mac.span(), "`VariadicArray<default!(T, default)>` not supported, choose `default!(VariadicArray<T>, ident)` instead"))?;
                        }
                        "composite_type" => {
                            let sql = Some(handle_composite_type_macro(mac)?);
                            let ty = syn::parse_quote! {
                                ::pgx::Array<::pgx::PgHeapTuple<'_, ::pgx::AllocatedByRust>>
                            };
                            Ok((ty, sql.map(|v| (v, wrapper_so_far))))
                        }
                        _ => Ok((syn::Type::Path(original), None)),
                    }
                }
                syn::Type::Path(arg_type_path) => {
                    let last = arg_type_path.path.segments.last().ok_or(syn::Error::new(
                        arg_type_path.span(),
                        "No last segment in type path",
                    ))?;
                    match last.ident.to_string().as_str() {
                        "Option" => {
                            let wrapper = match wrapper_so_far {
                                    CompositeTypeWrapper::OptionArray => CompositeTypeWrapper::OptionArrayOption,
                                    CompositeTypeWrapper::Array => CompositeTypeWrapper::ArrayOption,
                                    _ => return Err(syn::Error::new(last.span(), "Only Array<..>, Option<Array<..>, and Option<Array<Option<..>> are valid"))?,
                                };
                            resolve_option_inner(original, last.arguments.clone(), wrapper)
                        }
                        _ => Ok((syn::Type::Path(original), None)),
                    }
                }
                _ => Ok((syn::Type::Path(original), None)),
            },
            _ => Ok((syn::Type::Path(original), None)),
        },
        _ => Ok((syn::Type::Path(original), None)),
    }
}

fn resolve_option_inner(
    original: syn::TypePath,
    arguments: syn::PathArguments,
    wrapper_so_far: CompositeTypeWrapper,
) -> syn::Result<(syn::Type, Option<(syn::Expr, CompositeTypeWrapper)>)> {
    match arguments {
        syn::PathArguments::AngleBracketed(path_arg) => match path_arg.args.first() {
            Some(syn::GenericArgument::Type(ty)) => {
                match ty.clone() {
                    syn::Type::Macro(macro_pat) => {
                        let mac = &macro_pat.mac;
                        let archetype = mac.path.segments.last().expect("No last segment");
                        match archetype.ident.to_string().as_str() {
                            // Option<composite_type!(..)>
                            "composite_type" => {
                                let sql = Some(handle_composite_type_macro(mac)?);
                                let ty = syn::parse_quote! {
                                    Option<::pgx::PgHeapTuple<'_, ::pgx::AllocatedByRust>>
                                };
                                Ok((ty, sql.map(|v| (v, wrapper_so_far))))
                            },
                            // Option<default!(composite_type!(..))> isn't valid. If the user wanted the default to be `NULL` they just don't need a default.
                            "default" => return Err(syn::Error::new(mac.span(), "`Option<default!(T, default)>` not supported, choose `Option<T>` for a default of `NULL`, or `default!(T, default)` for a non-NULL default"))?,
                            _ => Ok((syn::Type::Path(original), None)),
                        }
                    }
                    syn::Type::Path(arg_type_path) => {
                        let last = arg_type_path.path.segments.last().ok_or(syn::Error::new(
                            arg_type_path.span(),
                            "No last segment in type path",
                        ))?;
                        match last.ident.to_string().as_str() {
                            // Option<Vec<composite_type!(..)>>
                            // Option<Vec<Option<composite_type!(..)>>>
                            "Vec" => {
                                let wrapper = match wrapper_so_far {
                                    CompositeTypeWrapper::None => CompositeTypeWrapper::Option,
                                    CompositeTypeWrapper::Option => CompositeTypeWrapper::OptionVec,
                                    CompositeTypeWrapper::OptionVec => CompositeTypeWrapper::OptionVecOption,
                                    _ => return Err(syn::Error::new(last.span(), "Only Option<..>, Option<Vec<..>>, Option<Vec<Option<..>> are valid"))?,
                                };
                                resolve_vec_inner(original, last.arguments.clone(), wrapper)
                            },
                            // Option<VariadicArray<composite_type!(..)>>
                            // Option<VariadicArray<Option<composite_type!(..)>>>
                            "VariadicArray" => {
                                let wrapper = match wrapper_so_far {
                                    CompositeTypeWrapper::None => CompositeTypeWrapper::Option,
                                    CompositeTypeWrapper::Option => CompositeTypeWrapper::OptionVariadicArray,
                                    CompositeTypeWrapper::OptionVariadicArray => CompositeTypeWrapper::OptionVariadicArrayOption,
                                    _ => return Err(syn::Error::new(last.span(), "Only Option<..>, Option<VariadicArray<..>>, Option<VariadicArray<Option<..>> are valid"))?,
                                };
                                resolve_variadic_array_inner(
                                    original,
                                    last.arguments.clone(),
                                    wrapper,
                                )
                            },
                            // Option<Array<composite_type!(..)>>
                            // Option<Array<Option<composite_type!(..)>>>
                            "Array" => {
                                let wrapper = match wrapper_so_far {
                                    CompositeTypeWrapper::None => CompositeTypeWrapper::Option,
                                    CompositeTypeWrapper::Option => CompositeTypeWrapper::OptionArray,
                                    CompositeTypeWrapper::OptionArray => CompositeTypeWrapper::OptionArrayOption,
                                    _ => return Err(syn::Error::new(last.span(), "Only Option<..>, Option<Array<..>>, Option<Array<Option<..>> are valid"))?,
                                };
                                resolve_array_inner(
                                    original,
                                    last.arguments.clone(),
                                    wrapper,
                                )
                            },
                            // Option<..>
                            _ => Ok((syn::Type::Path(original), None)),
                        }
                    }
                    _ => Ok((syn::Type::Path(original), None)),
                }
            }
            _ => Ok((syn::Type::Path(original), None)),
        },
        _ => Ok((syn::Type::Path(original), None)),
    }
}
