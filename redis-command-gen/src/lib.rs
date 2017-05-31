extern crate proc_macro;

extern crate syn;

#[macro_use]
extern crate quote;

use proc_macro::TokenStream;

use syn::{
    MetaItem,
    Ident,
    Lit,
};


#[proc_macro_derive(RedisCommandAttrs, attributes(command))]
pub fn gen_rediscommand(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_macro_input(&s).unwrap();
    let gen = impl_gen_redis_command(&ast);
    gen.parse().unwrap()
}

fn impl_gen_redis_command(ast: &syn::MacroInput) -> quote::Tokens {
    let struct_ident = &ast.ident;
    let attrs = &ast.attrs;

    // TODO extract from attrs
    //println!("impl_gen_redis_command attrs {:?}", attrs);

    let extern_func_ident = {
        let extern_func_name = format!("{}_RedisCommand", struct_ident);
        Ident::new(extern_func_name)
    };

    // Parse custom attributes, panic if the consumer hasn't provided them
    let (command_name, flags, static_name): (String, String, String) = {

        let mut cmd_name = "";
        let mut flags = "";
        let mut static_name = "";

        for attr in attrs.iter() {
            match &attr.value {
                &MetaItem::List(ref ident, ref items) => {
                    if *ident == Ident::new("command") {
                        for item in items {
                            //println!("found {:?}", item);
                            match item {
                                &syn::NestedMetaItem::MetaItem(MetaItem::NameValue(ref ident, Lit::Str(ref value, _))) => {
                                    //println!("Found command={}, value={}", ident, value);
                                    if *ident == Ident::new("name") {
                                        cmd_name = value;
                                    } else if *ident == Ident::new("flags") {
                                        flags = value;
                                    } else if *ident == Ident::new("static_handle") {
                                        static_name = value;
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                },
                _ => {}
            }
        }
        if cmd_name.is_empty() { panic!("Unable to find attr for command(name)")}
        if flags.is_empty() { panic!("Unable to find attr for command(flags)")}
        if static_name.is_empty() { panic!("Unable to find attr for command(static_handle)")}

        (cmd_name.to_string(), flags.to_string(), static_name.to_string())
    };


    let static_ident = Ident::new(static_name);

    let code: quote::Tokens = quote! {

        // Must be an empty tag struct
        static #static_ident: #struct_ident = #struct_ident;

        impl #struct_ident {
            fn register(
                &self,
                ctx: *mut RedisModuleCtx,
            ) -> raw::Status {

                let status = raw::create_command(
                   ctx,
                   format!("{}\0", self.name()).as_ptr(),
                   Some(#extern_func_ident),
                   format!("{}\0", self.str_flags()).as_ptr(),
                   0,
                   0,
                   0
                );
                //println!("create_command: {}, err: {:?}", self.name(), status );
                status
            }
        }

        impl RedisCommandAttrs for #struct_ident {
            fn name(&self) -> &'static str { #command_name }
            fn str_flags(&self) -> &'static str { #flags }
        }

        #[allow(non_snake_case)]
        #[allow(unused_variables)]
        #[no_mangle]
        pub extern "C" fn #extern_func_ident(
            ctx: *mut raw::RedisModuleCtx,
            argv: *mut *mut raw::RedisModuleString,
            argc: c_int
        ) -> raw::Status {
            RedisCommand::harness(&#static_ident, ctx, argv, argc)
        }

    };
    //println!("generated code\n{}", code);
    code
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
    }
}

