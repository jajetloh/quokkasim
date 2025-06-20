use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type};

#[proc_macro_derive(WithMethods)]
pub fn derive_with_methods(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    
    let mut methods = Vec::new();
    methods.push(generate_new_struct_method());

    if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            for field in &fields_named.named {
                let field_name = field.ident.as_ref().unwrap();
                let field_name_str = field_name.to_string();
                let field_type = &field.ty;
                
                match field_name_str.as_str() {
                    "element_name" => {
                        methods.push(generate_simple_with_method("with_name", field_name, field_type));
                    },
                    "element_code" => {
                        methods.push(generate_simple_with_method("with_code", field_name, field_type));
                    },
                    "element_type" => {
                        methods.push(generate_simple_with_method("with_type", field_name, field_type));
                    },
                    "resource" => {
                        methods.push(quote! {
                            pub fn with_initial_resource(mut self, resource: #field_type) -> Self {
                                self.resource = resource;
                                self
                            }
                        })
                    },
                    "source_vector" => {
                        methods.push(generate_with_and_inplace_method("source_vector", field_name, field_type));
                    },
                    "delay_modes" => {
                        methods.push(quote! {
                            pub fn with_delay_mode(mut self, delay_mode_change: DelayModeChange) -> Self {
                                self.delay_modes.modify(delay_mode_change);
                                self
                            }
                            
                            pub fn with_delay_mode_inplace(&mut self, delay_mode_change: DelayModeChange) {
                                self.delay_modes.modify(delay_mode_change);
                            }
                        });
                    },
                    "process_quantity_distr" => {
                        methods.push(generate_with_and_inplace_method("process_quantity_distr", field_name, field_type));
                    },
                    "process_time_distr" => {
                        // methods.push(generate_simple_with_method("with_process_time_distr", field_name, field_type));
                        methods.push(generate_with_and_inplace_method("process_time_distr", field_name, field_type));
                    },
                    "low_capacity" => {
                        methods.push(generate_with_and_inplace_method("low_capacity", field_name, field_type));
                    },
                    "max_capacity" => {
                        methods.push(generate_with_and_inplace_method("max_capacity", field_name, field_type));
                    },
                    "item_factory" => {
                        methods.push(generate_with_and_inplace_method("item_factory", field_name, field_type));
                    },
                    _ => {
                        // // Generate a generic with_fieldname method for any other field
                        // let method_name = format!("with_{}", field_name_str);
                        // let method_ident = syn::Ident::new(&method_name, field_name.span());
                        // methods.push(quote! {
                        //     pub fn #method_ident(self, #field_name: #field_type) -> Self {
                        //         Self {
                        //             #field_name,
                        //             ..self
                        //         }
                        //     }
                        // });
                    }
                }
            }
        }
    }
    
    let expanded = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #(#methods)*
        }
    };
    
    TokenStream::from(expanded)
}

fn generate_new_struct_method() -> proc_macro2::TokenStream {
    quote! {
        pub fn new() -> Self where Self: Default {
            Self::default()
        }
    }
}

fn generate_simple_with_method(
    method_name: &str, 
    field_name: &syn::Ident, 
    field_type: &Type
) -> proc_macro2::TokenStream {
    let method_ident = syn::Ident::new(method_name, field_name.span());
    quote! {
        pub fn #method_ident(self, #field_name: #field_type) -> Self {
            Self {
                #field_name,
                ..self
            }
        }
    }
}

fn generate_with_and_inplace_method(
    base_name: &str,
    field_name: &syn::Ident, 
    field_type: &Type
) -> proc_macro2::TokenStream {
    let with_method = format!("with_{}", base_name);
    let inplace_method = format!("with_{}_inplace", base_name);
    
    let with_ident = syn::Ident::new(&with_method, field_name.span());
    let inplace_ident = syn::Ident::new(&inplace_method, field_name.span());
    
    quote! {
        pub fn #with_ident(self, #field_name: #field_type) -> Self {
            Self {
                #field_name,
                ..self
            }
        }
        
        pub fn #inplace_ident(&mut self, #field_name: #field_type) {
            self.#field_name = #field_name;
        }
    }
}