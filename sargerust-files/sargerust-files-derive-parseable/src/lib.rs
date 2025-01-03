extern crate proc_macro2;

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Data, DeriveInput, Fields, Ident, parse_macro_input, spanned::Spanned};

#[proc_macro_derive(Parse)]
pub fn derive_parseable(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    derive_parse_internal(input).into()
}

// taken from sharnoff/derive-syn-parse: put it into a separate function for testability
pub(crate) fn derive_parse_internal(input: DeriveInput) -> TokenStream {
    let found_crate = crate_name("sargerust-files").expect("sargerust-files is present in `Cargo.toml`");

    let crate_name = match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(#ident)
        }
    };

    let ident = input.ident;
    let parse_impl = match input.data {
        Data::Union(_) => panic!("`#[derive(Parse)]` is only available on structs: {}", ident),
        Data::Struct(s) => match s.fields {
            Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    let ftype = &f.ty;
                    // TODO: when ftype is generic (especially Vec<T>), we need to change the token to Vec::<T>
                    // see e.g. https://blog.turbo.fish/proc-macro-simple-derive/
                    // try to match ftype into Path(https://docs.rs/syn/1.0.109/syn/struct.TypePath.html)
                    // get https://docs.rs/syn/1.0.109/syn/enum.PathArguments.html
                    // something like:

                    // let ftype = match &f.ty {
                    //     Type::Path(path) => {
                    //         match &path.path.segments.first()?.arguments {
                    //             AngleBracketed(angle) => angle.args.first()
                    //             _ => &f.ty
                    //         }
                    //     },
                    //     _ => &f.ty
                    // };

                    quote_spanned! {f.span()=>
                        #name: #ftype::parse(rdr)?,
                    }
                });
                quote! { #(#recurse)* }
            }
            _ => panic!(
                "#[derive(Parse)]` only supports named struct fields at the moment: {}",
                ident
            ),
        },
        Data::Enum(_) => panic!("`#[derive(Parse)]` is only available on structs: {}", ident),
    };

    quote!(
        impl #crate_name::common::reader::Parseable<#ident> for #ident {
            fn parse<R: Read>(rdr: &mut R) -> Result<#ident, #crate_name::ParserError> {
                Ok(#ident{
                    #parse_impl
                })
            }
        }

        impl #crate_name::common::reader::Parseable<Vec<#ident>> for Vec<#ident> {
            fn parse<R: Read>(rdr: &mut R) -> Result<Vec<#ident>, #crate_name::ParserError> {
                Ok(#crate_name::common::reader::read_chunk_array(rdr)?)
            }
        }
    )
}
