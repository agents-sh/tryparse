//! Struct code generation for LlmDeserialize derive macro.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, GenericArgument, PathArguments, Type};

/// Generate deserialization code for structs with named fields.
pub fn generate_struct_deserialize(
    name: &syn::Ident,
    data: &syn::DataStruct,
    tryparse_crate: &TokenStream,
) -> TokenStream {
    match &data.fields {
        Fields::Named(fields) => {
            let field_names: Vec<_> = fields.named.iter().map(|f| &f.ident).collect();
            let field_types: Vec<_> = fields.named.iter().map(|f| &f.ty).collect();
            let field_name_strs: Vec<_> = fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect();

            // Check if each field is Option<T>
            let is_optional: Vec<_> = field_types.iter().map(|ty| is_option_type(ty)).collect();

            // Extract inner type for Option<T> fields
            let inner_types: Vec<_> = field_types
                .iter()
                .zip(&is_optional)
                .map(|(ty, opt)| {
                    if *opt {
                        extract_option_inner(ty)
                    } else {
                        (*ty).clone()
                    }
                })
                .collect();

            let name_str = name.to_string();

            // Generate field descriptor setup (collect to Vec for reuse)
            let field_descriptors: Vec<_> = field_name_strs
                .iter()
                .zip(&field_types)
                .zip(&is_optional)
                .map(|((name, ty), opt)| {
                    let type_name = quote!(stringify!(#ty)).to_string();
                    quote! {
                        .field(#tryparse_crate::deserializer::FieldDescriptor::new(
                            #name,
                            #type_name,
                            #opt
                        ))
                    }
                })
                .collect();

            // Generate field extraction for try_deserialize (returns Option)
            let field_extractions_strict: Vec<_> = field_names
                .iter()
                .zip(&inner_types)
                .zip(&is_optional)
                .map(|((field_name, inner_ty), opt)| {
                    let field_name_str = field_name.as_ref().unwrap().to_string();
                    if *opt {
                        // Optional field
                        quote! {
                            let #field_name = fields.get(#field_name_str)
                                .and_then(|v| v.downcast_ref::<#inner_ty>())
                                .cloned();
                        }
                    } else {
                        // Required field - return None if missing
                        quote! {
                            let #field_name = fields.get(#field_name_str)
                                .and_then(|v| v.downcast_ref::<#inner_ty>())
                                .cloned()?;
                        }
                    }
                })
                .collect();

            // Generate field extraction for deserialize (returns Result)
            let field_extractions_lenient: Vec<_> = field_names.iter().zip(&inner_types).zip(&is_optional).map(|((field_name, inner_ty), opt)| {
                let field_name_str = field_name.as_ref().unwrap().to_string();
                if *opt {
                    // Optional field
                    quote! {
                        let #field_name = fields.get(#field_name_str)
                            .and_then(|v| v.downcast_ref::<#inner_ty>())
                            .cloned();
                    }
                } else {
                    // Required field
                    quote! {
                        let #field_name = fields.get(#field_name_str)
                            .and_then(|v| v.downcast_ref::<#inner_ty>())
                            .cloned()
                            .ok_or_else(|| #tryparse_crate::error::ParseError::DeserializeFailed(
                                #tryparse_crate::error::DeserializeError::missing_field(#field_name_str)
                            ))?;
                    }
                }
            }).collect();

            quote! {
                fn try_deserialize(
                    value: &#tryparse_crate::value::FlexValue,
                    ctx: &mut #tryparse_crate::deserializer::CoercionContext,
                ) -> Option<Self> {
                    use std::any::Any;

                    let mut deserializer = #tryparse_crate::deserializer::StructDeserializer::new()
                        #(#field_descriptors)*;

                    let fields = deserializer.try_deserialize(
                        value,
                        ctx,
                        #name_str,
                        |field_name, field_value, field_ctx| {
                            // Dispatch to the appropriate field type's LlmDeserialize impl (strict mode only)
                            match field_name {
                                #(
                                    #field_name_strs => {
                                        // Try strict deserialization
                                        <#inner_types as #tryparse_crate::deserializer::LlmDeserialize>::try_deserialize(field_value, field_ctx)
                                            .map(|v| Box::new(v) as Box<dyn Any>)
                                    }
                                )*
                                _ => None
                            }
                        }
                    ).ok()?;

                    // Extract fields from Box<dyn Any> (strict mode - return None on failure)
                    #(#field_extractions_strict)*

                    Some(Self {
                        #(#field_names),*
                    })
                }

                fn deserialize(
                    value: &#tryparse_crate::value::FlexValue,
                    ctx: &mut #tryparse_crate::deserializer::CoercionContext,
                ) -> #tryparse_crate::error::Result<Self> {
                    use std::any::Any;

                    let mut deserializer = #tryparse_crate::deserializer::StructDeserializer::new()
                        #(#field_descriptors)*;

                    let fields = deserializer.deserialize(
                        value,
                        ctx,
                        #name_str,
                        |field_name, field_value, field_ctx, strict| {
                            // Dispatch to the appropriate field type's LlmDeserialize impl
                            match field_name {
                                #(
                                    #field_name_strs => {
                                        if strict {
                                            // Try strict deserialization
                                            if let Some(v) = <#inner_types as #tryparse_crate::deserializer::LlmDeserialize>::try_deserialize(field_value, field_ctx) {
                                                Ok(Box::new(v) as Box<dyn Any>)
                                            } else {
                                                Err(#tryparse_crate::error::ParseError::DeserializeFailed(
                                                    #tryparse_crate::error::DeserializeError::type_mismatch(
                                                        stringify!(#inner_types),
                                                        "value"
                                                    )
                                                ))
                                            }
                                        } else {
                                            // Lenient deserialization
                                            let v = <#inner_types as #tryparse_crate::deserializer::LlmDeserialize>::deserialize(field_value, field_ctx)?;
                                            Ok(Box::new(v) as Box<dyn Any>)
                                        }
                                    }
                                )*
                                _ => Err(#tryparse_crate::error::ParseError::DeserializeFailed(
                                    #tryparse_crate::error::DeserializeError::Custom(
                                        format!("Unknown field: {}", field_name)
                                    )
                                ))
                            }
                        }
                    )?;

                    // Extract fields from Box<dyn Any> (lenient mode - return error on failure)
                    #(#field_extractions_lenient)*

                    Ok(Self {
                        #(#field_names),*
                    })
                }
            }
        }
        Fields::Unnamed(_) => syn::Error::new_spanned(
            data.fields.clone(),
            "LlmDeserialize does not support tuple structs yet",
        )
        .to_compile_error(),
        Fields::Unit => syn::Error::new_spanned(
            data.fields.clone(),
            "LlmDeserialize does not support unit structs",
        )
        .to_compile_error(),
    }
}

/// Check if a type is Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

/// Extract the inner type T from Option<T>
fn extract_option_inner(ty: &Type) -> Type {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return inner.clone();
                    }
                }
            }
        }
    }
    // Fallback: return the original type
    ty.clone()
}
