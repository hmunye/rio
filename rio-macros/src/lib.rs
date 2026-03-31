//! `rio` Procedural Macros.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

/// Marks "main" as an `async` function to be executed by the `rio` runtime.
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let sig = &input.sig;

    if sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            sig.fn_token,
            "`#[rio::main]` must be used on an `async` function",
        )
        .to_compile_error()
        .into();
    }

    if sig.ident != "main" {
        return syn::Error::new_spanned(
            &sig.ident,
            "`#[rio::main]` must be used on the `main` function",
        )
        .to_compile_error()
        .into();
    }

    if !sig.inputs.is_empty() {
        return syn::Error::new_spanned(&sig.inputs, "`main` function cannot accept arguments")
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
