//! `rio` Procedural Macros.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, ItemFn, parse_macro_input};

/// Marks an `async` function as an entry point for a `rio` runtime.
///
/// This macro can be used on functions other than `main`. In that case, it
/// creates a new runtime per-call and executes synchronously.
///
/// # Examples
///
/// ```rust,ignore
/// #[rio::main]
/// async fn main() {
///     // ...
/// }
/// ```
///
/// This expands to code equivalent to:
///
/// ```rust,ignore
/// fn main() {
///     let rt = rio::rt::Runtime::new();
///     rt.block_on(async {
///         // ...
///     })
/// }
/// ```
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let sig = &input.sig;

    if sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            sig.fn_token,
            "`async` keyword is missing from the function declaration",
        )
        .to_compile_error()
        .into();
    }

    let mut transformed = sig.clone();
    transformed.asyncness = None;

    let attrs = &input.attrs;
    let vis = &input.vis;
    let block = &input.block;

    TokenStream::from(quote! {
        #(#attrs)*
        #vis #transformed {
            let rt = rio::rt::Runtime::new();
            rt.block_on(async #block)
        }
    })
}

/// Marks an `async` test function as an entry point for a `rio` runtime.
///
/// # Examples
///
/// ```rust,ignore
/// #[rio::test]
/// async fn my_async_test() {
///     // ...
/// }
/// ```
///
/// This expands to code equivalent to:
///
/// ```rust,ignore
/// #[test]
/// fn my_async_test() {
///     let rt = rio::rt::Runtime::new();
///     rt.block_on(async {
///         // ...
///     })
/// }
/// ```
#[proc_macro_attribute]
pub fn test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    if let Some(attr) = input.attrs.iter().find(|attr| is_test_attribute(attr)) {
        return syn::Error::new_spanned(
            attr,
            "second test attribute is supplied, consider removing or changing the order of your test attributes"
        )
        .to_compile_error()
        .into();
    }

    let sig = &input.sig;

    if sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            sig.fn_token,
            "`async` keyword is missing from the function declaration",
        )
        .to_compile_error()
        .into();
    }

    if !sig.inputs.is_empty() {
        return syn::Error::new_spanned(&sig.inputs, "test functions cannot accept arguments")
            .to_compile_error()
            .into();
    }

    let mut transformed = sig.clone();
    transformed.asyncness = None;

    let attrs = &input.attrs;
    let vis = &input.vis;
    let block = &input.block;

    TokenStream::from(quote! {
        #(#attrs)*
        #[test]
        #vis #transformed {
            let rt = rio::rt::Runtime::new();
            rt.block_on(async #block)
        }
    })
}

/// Check whether given attribute is a test attribute of forms:
///
/// * `#[test]`
/// * `#[core::prelude::*::test]` or `#[::core::prelude::*::test]`
/// * `#[std::prelude::*::test]` or `#[::std::prelude::*::test]`
fn is_test_attribute(attr: &Attribute) -> bool {
    // <https://docs.rs/tokio-macros/2.6.1/src/tokio_macros/entry.rs.html#610>
    let path = match &attr.meta {
        syn::Meta::Path(path) => path,
        _ => return false,
    };

    let candidates = [
        ["core", "prelude", "*", "test"],
        ["std", "prelude", "*", "test"],
    ];

    if path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].arguments.is_none()
        && path.segments[0].ident == "test"
    {
        return true;
    } else if path.segments.len() != candidates[0].len() {
        return false;
    }

    candidates.into_iter().any(|segments| {
        path.segments.iter().zip(segments).all(|(segment, path)| {
            segment.arguments.is_none() && (path == "*" || segment.ident == path)
        })
    })
}
