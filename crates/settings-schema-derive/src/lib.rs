use heck::ToUpperCamelCase;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::meta::ParseNestedMeta;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Data, DeriveInput, Expr, Field, Fields, Ident, Lit, LitInt, LitStr, Token, Type};

/// Derives `settings_schema::Settings` for a struct with named fields and generates a
/// `<Struct>Field` enum with one variant per field marked `#[setting(..)]`. Each marked
/// field takes exactly one kind and an optional label:
///
/// ```text
/// #[setting(toggle)]                                        bool fields
/// #[setting(number(min = 0.0, max = 1.0, step = 0.05, precision = 2))]
/// #[setting(choice("stable" => "channel_stable", 30 => "30 FPS"))]
/// #[setting(toggle, label = "start_on_boot")]
/// ```
///
/// Number fields are converted with `as`, so any primitive numeric type works; bounds,
/// step and precision are optional and accept constant expressions. Choice fields go
/// through `Display` and `FromStr`.
#[proc_macro_derive(Settings, attributes(setting))]
pub fn derive_settings(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

struct Setting<'a> {
    ident: &'a Ident,
    ty: &'a Type,
    label: Option<LitStr>,
    kind: Kind,
}

enum Kind {
    Toggle,
    Number(Box<NumberArgs>),
    Choice(Vec<ChoiceArg>),
}

#[derive(Default)]
struct NumberArgs {
    min: Option<Expr>,
    max: Option<Expr>,
    step: Option<Expr>,
    precision: Option<LitInt>,
}

struct ChoiceArg {
    value: String,
    label: LitStr,
}

impl Parse for ChoiceArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let value = match input.parse()? {
            Lit::Str(value) => value.value(),
            Lit::Int(value) => value.base10_digits().to_string(),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "expected a string or integer value",
                ));
            }
        };
        input.parse::<Token![=>]>()?;
        Ok(Self {
            value,
            label: input.parse()?,
        })
    }
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "Settings can only be derived for structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "Settings needs a struct with named fields",
        ));
    };
    let settings = fields
        .named
        .iter()
        .filter_map(|field| parse_setting(field).transpose())
        .collect::<syn::Result<Vec<_>>>()?;
    if settings.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "no field is marked with #[setting(..)]",
        ));
    }

    let vis = &input.vis;
    let field_enum = format_ident!("{}Field", name);
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let variants: Vec<Ident> = settings
        .iter()
        .map(|setting| {
            Ident::new(
                &setting.ident.unraw().to_string().to_upper_camel_case(),
                setting.ident.span(),
            )
        })
        .collect();
    let count = settings.len();
    let schemas = settings.iter().map(schema_tokens);
    let get_arms = settings.iter().zip(&variants).map(|(setting, variant)| {
        let ident = setting.ident;
        let value = match setting.kind {
            Kind::Toggle => quote!(::settings_schema::Value::Bool(self.#ident)),
            Kind::Number(_) => quote!(::settings_schema::Value::Number(self.#ident as f64)),
            Kind::Choice(_) => quote! {
                ::settings_schema::Value::Text(::std::string::ToString::to_string(&self.#ident))
            },
        };
        quote!(#field_enum::#variant => #value)
    });
    let set_arms = settings.iter().zip(&variants).map(|(setting, variant)| {
        let ident = setting.ident;
        let ty = setting.ty;
        match setting.kind {
            Kind::Toggle => quote! {
                (#field_enum::#variant, ::settings_schema::Value::Bool(value)) => self.#ident = value
            },
            Kind::Number(_) => quote! {
                (#field_enum::#variant, ::settings_schema::Value::Number(value)) => {
                    self.#ident = value as #ty
                }
            },
            Kind::Choice(_) => quote! {
                (#field_enum::#variant, ::settings_schema::Value::Text(value)) => {
                    match value.parse::<#ty>() {
                        ::core::result::Result::Ok(value) => self.#ident = value,
                        ::core::result::Result::Err(_) => return false,
                    }
                }
            },
        }
    });

    Ok(quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #vis enum #field_enum {
            #(#variants,)*
        }

        impl #impl_generics ::settings_schema::Settings for #name #type_generics #where_clause {
            type Field = #field_enum;

            const FIELDS: &'static [#field_enum] = &[#(#field_enum::#variants),*];

            fn schema(field: #field_enum) -> &'static ::settings_schema::Schema {
                static SCHEMAS: [::settings_schema::Schema; #count] = [#(#schemas),*];
                &SCHEMAS[field as usize]
            }

            fn get(&self, field: #field_enum) -> ::settings_schema::Value {
                match field {
                    #(#get_arms,)*
                }
            }

            fn set(&mut self, field: #field_enum, value: ::settings_schema::Value) -> bool {
                let ::core::option::Option::Some(value) = Self::schema(field).kind.normalize(value)
                else {
                    return false;
                };
                match (field, value) {
                    #(#set_arms,)*
                    _ => return false,
                }
                true
            }
        }
    })
}

fn parse_setting(field: &Field) -> syn::Result<Option<Setting<'_>>> {
    let Some(ident) = &field.ident else {
        return Ok(None);
    };
    let mut attrs = field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("setting"))
        .peekable();
    let Some(first) = attrs.peek().copied() else {
        return Ok(None);
    };
    let mut label = None;
    let mut kind = None;
    for attr in attrs {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("label") {
                label = Some(meta.value()?.parse()?);
                return Ok(());
            }
            let parsed = if meta.path.is_ident("toggle") {
                Kind::Toggle
            } else if meta.path.is_ident("number") {
                Kind::Number(Box::new(parse_number(&meta)?))
            } else if meta.path.is_ident("choice") {
                let content;
                syn::parenthesized!(content in meta.input);
                let choices = Punctuated::<ChoiceArg, Token![,]>::parse_terminated(&content)?;
                Kind::Choice(choices.into_iter().collect())
            } else {
                return Err(
                    meta.error("expected `toggle`, `number(..)`, `choice(..)` or `label = \"..\"`")
                );
            };
            if kind.replace(parsed).is_some() {
                return Err(meta.error("a setting takes exactly one kind"));
            }
            Ok(())
        })?;
    }
    let Some(kind) = kind else {
        return Err(syn::Error::new_spanned(
            first,
            "a setting needs `toggle`, `number(..)` or `choice(..)`",
        ));
    };
    Ok(Some(Setting {
        ident,
        ty: &field.ty,
        label,
        kind,
    }))
}

fn parse_number(meta: &ParseNestedMeta) -> syn::Result<NumberArgs> {
    let mut args = NumberArgs::default();
    meta.parse_nested_meta(|arg| {
        if arg.path.is_ident("min") {
            args.min = Some(arg.value()?.parse()?);
        } else if arg.path.is_ident("max") {
            args.max = Some(arg.value()?.parse()?);
        } else if arg.path.is_ident("step") {
            args.step = Some(arg.value()?.parse()?);
        } else if arg.path.is_ident("precision") {
            args.precision = Some(arg.value()?.parse()?);
        } else {
            return Err(arg.error("expected `min`, `max`, `step` or `precision`"));
        }
        Ok(())
    })?;
    Ok(args)
}

fn schema_tokens(setting: &Setting) -> TokenStream2 {
    let key = setting.ident.unraw().to_string();
    let label = setting
        .label
        .as_ref()
        .map_or_else(|| key.clone(), LitStr::value);
    let kind = match &setting.kind {
        Kind::Toggle => quote!(::settings_schema::Kind::Toggle),
        Kind::Number(args) => {
            let bound = |value: &Option<Expr>, default: TokenStream2| {
                value
                    .as_ref()
                    .map_or(default, |value| quote!((#value) as f64))
            };
            let min = bound(&args.min, quote!(f64::NEG_INFINITY));
            let max = bound(&args.max, quote!(f64::INFINITY));
            let step = bound(&args.step, quote!(1.0));
            let precision = args
                .precision
                .as_ref()
                .map_or_else(|| quote!(0), |precision| quote!(#precision));
            quote! {
                ::settings_schema::Kind::Number(::settings_schema::Number {
                    min: #min,
                    max: #max,
                    step: #step,
                    precision: #precision,
                })
            }
        }
        Kind::Choice(choices) => {
            let values = choices.iter().map(|choice| &choice.value);
            let labels = choices.iter().map(|choice| &choice.label);
            quote! {
                ::settings_schema::Kind::Choice(&[
                    #(::settings_schema::Choice { value: #values, label: #labels }),*
                ])
            }
        }
    };
    quote! {
        ::settings_schema::Schema {
            key: #key,
            label: #label,
            kind: #kind,
        }
    }
}
