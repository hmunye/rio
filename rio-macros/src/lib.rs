//! `rio` Procedural Macros.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, ItemFn, parse_macro_input};

/// Marks the `async` "main" function to be executed by the `rio` runtime.
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let sig = &input.sig;

    if sig.asyncness.is_none() {
        return syn::Error::new_spanned(sig.fn_token, "must be used on an `async` function")
            .to_compile_error()
            .into();
    }

    if sig.ident != "main" {
        return syn::Error::new_spanned(&sig.ident, "must be used on the main function")
            .to_compile_error()
            .into();
    }

    if !sig.inputs.is_empty() {
        return syn::Error::new_spanned(&sig.inputs, "main function cannot accept arguments")
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

// Check whether given attribute is a test attribute of forms:
//
// * `#[test]`
// * `#[core::prelude::*::test]` or `#[::core::prelude::*::test]`
// * `#[std::prelude::*::test]` or `#[::std::prelude::*::test]`
//
// <https://docs.rs/tokio-macros/2.6.1/src/tokio_macros/entry.rs.html#610>
fn is_test_attribute(attr: &Attribute) -> bool {
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

/// Marks an `async` function to be executed by the `rio` runtime, suitable for
/// test environments.
#[proc_macro_attribute]
pub fn test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    if let Some(attr) = input.attrs.iter().find(|attr| is_test_attribute(attr)) {
        return syn::Error::new_spanned(
            attr,
            "second test attribute supplied, consider removing or changing the order of your test attributes"
        )
        .to_compile_error()
        .into();
    }

    let sig = &input.sig;

    if sig.asyncness.is_none() {
        return syn::Error::new_spanned(sig.fn_token, "must be used on an `async` function")
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
