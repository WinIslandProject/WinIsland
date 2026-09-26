use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, ReturnType, Type, parse_macro_input, parse_quote};

/// `#[c_result(CType)]` lets an `extern "C"` function be written against a Rust `Result`
/// and use `?`, while the exported symbol returns `CType`. The value is converted with
/// `From<Result<..>> for CType`, so the error type only needs whatever that impl accepts.
#[proc_macro_attribute]
pub fn c_result(attr: TokenStream, item: TokenStream) -> TokenStream {
    let c_type = parse_macro_input!(attr as Type);
    let ItemFn {
        attrs,
        vis,
        mut sig,
        block,
    } = parse_macro_input!(item as ItemFn);
    let ReturnType::Type(_, result_type) = sig.output.clone() else {
        return syn::Error::new_spanned(sig.fn_token, "c_result needs a function returning Result")
            .to_compile_error()
            .into();
    };
    sig.output = parse_quote!(-> #c_type);
    quote! {
        #(#attrs)*
        #vis #sig {
            <#c_type as ::core::convert::From<#result_type>>::from((move || -> #result_type #block)())
        }
    }
    .into()
}
