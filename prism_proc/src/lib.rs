use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericArgument, Index, ItemImpl, Member, PathArguments};
use syn::{Error, Type, parse_macro_input};

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[proc_macro_derive(Component, attributes(skip, on_dyn_event, listen))]
pub fn derive_component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut dynamic = false;
    let mut custom_listen = false;
    for attribute in &input.attrs {
        let (name, enabled) = if attribute.path().is_ident("on_dyn_event") {
            ("on_dyn_event", &mut dynamic)
        } else if attribute.path().is_ident("listen") {
            ("listen", &mut custom_listen)
        } else if attribute.path().is_ident("on_change") {
            return Error::new_spanned(
                attribute, "place #[on_change(source)] on an OnChange implementation"
            ).to_compile_error().into();
        } else {continue};
        if !matches!(&attribute.meta, syn::Meta::Path(_)) {
            return Error::new_spanned(attribute, format!("use #[{name}] without arguments"))
                .to_compile_error().into();
        }
        if *enabled {
            return Error::new_spanned(attribute, format!("duplicate #[{name}]"))
                .to_compile_error().into();
        }
        *enabled = true;
    }

    let (children, children_mut, layout, listen, update) = match &input.data {
        Data::Struct(data) => {
            let mut fields = data.fields.iter().enumerate();
            let (_, first) = fields.next()
                .expect("Component requires the first field to be the layout");
            let layout = first.ident.clone().map(Member::Named)
                .unwrap_or(Member::Unnamed(Index::from(0)));
            let members: Vec<_> = fields
                .filter(|(_, field)| {!field.attrs.iter().any(|attr| attr.path().is_ident("skip"))})
                .map(|(index, field)| {
                    field.ident.clone().map(Member::Named)
                        .unwrap_or(Member::Unnamed(Index::from(index)))
                }).collect();
            assert!(!members.is_empty(), "Component requires at least one child");
            let count = members.len();
            let (listen, update) = match ChangeFields::expand(&data.fields, false) {
                Ok(methods) => methods,
                Err(error) => return error.to_compile_error().into(),
            };

            (
                quote! {
                    let children: [Vec<_>; #count] = [
                        #(prism::drawable::Drawables::drawables(&self.#members)),*
                    ];
                    children.into_iter().flatten().collect()
                },
                quote! {
                    let children: [Vec<_>; #count] = [
                        #(prism::drawable::Drawables::drawables_mut(&mut self.#members)),*
                    ];
                    children.into_iter().flatten().collect()
                },
                quote! {&self.#layout},
                listen, update,
            )
        }
        Data::Enum(data) => {
            let mut children_arms = Vec::new();
            let mut children_mut_arms = Vec::new();
            let mut layout_arms = Vec::new();
            let mut listen_arms = Vec::new();
            let mut update_arms = Vec::new();

            for variant in &data.variants {
                let variant_name = &variant.ident;
                let fields = match &variant.fields {
                    Fields::Named(fields) => &fields.named,
                    _ => panic!("Only named enum variants are supported"),
                };
                let mut fields = fields.iter().enumerate();
                let (_, first) = fields.next()
                    .expect("Component requires the first field to be the layout");
                let layout = first.ident.as_ref().unwrap();
                let (members, bindings): (Vec<_>, Vec<_>) = fields
                    .filter(|(_, field)| {
                        !field.attrs.iter().any(|attr| attr.path().is_ident("skip"))
                    }).map(|(index, field)| {
                        (field.ident.as_ref().unwrap(), format_ident!("__prism_child_{}", index))
                    }).unzip();
                let count = members.len();
                let (listen, update) = match ChangeFields::expand(&variant.fields, true) {
                    Ok(methods) => methods,
                    Err(error) => return error.to_compile_error().into(),
                };
                let (source_members, source_bindings): (Vec<_>, Vec<_>) =
                    variant.fields.iter().enumerate().skip(1).map(|(index, field)| {
                        (field.ident.as_ref().unwrap(), format_ident!("__prism_child_{}", index))
                    }).unzip();
                listen_arms.push(quote! {
                    Self::#variant_name {#(#source_members: #source_bindings,)* ..} => {#listen}
                });
                update_arms.push(quote! {
                    Self::#variant_name {#(#source_members: #source_bindings,)* ..} => {#update}
                });

                children_arms.push(quote! {
                    Self::#variant_name {#(#members: #bindings,)* ..} => {
                        let children: [Vec<_>; #count] = [
                            #(prism::drawable::Drawables::drawables(#bindings)),*
                        ];
                        children.into_iter().flatten().collect()
                    }
                });
                children_mut_arms.push(quote! {
                    Self::#variant_name {#(#members: #bindings,)* ..} => {
                        let children: [Vec<_>; #count] = [
                            #(prism::drawable::Drawables::drawables_mut(#bindings)),*
                        ];
                        children.into_iter().flatten().collect()
                    }
                });
                layout_arms.push(quote! {Self::#variant_name {#layout, ..} => #layout});
            }

            (
                quote! {match self {#(#children_arms),*}},
                quote! {match self {#(#children_mut_arms),*}},
                quote! {match self {#(#layout_arms),*}},
                quote! {match self {#(#listen_arms),*}},
                quote! {match self {#(#update_arms),*}},
            )
        }
        Data::Union(_) => panic!("Cannot implement Component for a union"),
    };

    let (listen, update) = if custom_listen {
        (quote! {Self::listen(self)}, quote! {Self::update(self, ctx, path, update)})
    } else {(listen, update)};
    let sources = match &input.data {
        Data::Struct(data) => {
            let methods = ChangeFields::sources(&data.fields);
            quote! {
                #[allow(dead_code)]
                impl #impl_generics #name #ty_generics #where_clause {#methods}
            }
        }
        _ => quote! {},
    };
    let on_event = dynamic.then(|| quote! {
        fn dispatch_event(&mut self, ctx: &mut prism::Context, event: Box<dyn prism::events::DynEvent>) -> Vec<Box<dyn prism::events::DynEvent>> {
            Self::on_event(self, ctx, event)
        }
    });

    quote! {
        impl #impl_generics prism::drawable::Component for #name #ty_generics #where_clause {
            fn children(&self) -> Vec<&dyn prism::drawable::Drawable> {#children}
            fn children_mut(&mut self) -> Vec<&mut dyn prism::drawable::Drawable> {#children_mut}
            fn layout(&self) -> &dyn prism::drawable::Layout {#layout}
            fn listen(&mut self) -> ::prism::events::ListenFuture<'_> {#listen}
            fn update(&mut self, ctx: &mut prism::Context, mut path: Vec<usize>, update: Box<dyn ::std::any::Any>) {
                #update
            }
            #on_event
        }
        #sources
    }.into()
}

#[proc_macro_attribute]
pub fn on_event(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    Registration::expand(args, input)
}

#[proc_macro_attribute]
pub fn on_change(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    Registration::change(args, input)
}

struct ChangeFields;
impl ChangeFields {
    fn expand(fields: &Fields, bindings: bool) -> syn::Result<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
        let mut waits = Vec::new();
        let mut updates = Vec::new();
        for (index, field) in fields.iter().enumerate() {
            if let Some(attribute) = field.attrs.iter()
                .find(|attr| attr.path().is_ident("on_change")) {
                return Err(Error::new_spanned(
                    attribute, "place #[on_change(source)] on an OnChange implementation"
                ))
            }
            if index == 0 {continue}
            let skipped = field.attrs.iter().any(|attr| attr.path().is_ident("skip"));
            let member = field.ident.clone().map(Member::Named)
                .unwrap_or(Member::Unnamed(Index::from(index)));
            let field_name = quote!(#member).to_string().replace("r#", "");
            let binding = format_ident!("__prism_child_{}", index);
            let reference = if bindings {quote!(#binding)} else {quote!(&mut self.#member)};
            let children = (!skipped).then(|| quote! {
                for child in ::prism::drawable::Drawables::drawables_mut(#reference) {
                    listeners.push(::prism::drawable::Drawable::listen(child));
                }
            });
            let child_updates = (!skipped).then(|| quote! {
                for child in ::prism::drawable::Drawables::drawables_mut(#reference) {
                    if index == selected {
                        child.update(ctx, path, update);
                        return
                    }
                    index += 1;
                }
            });
            waits.push(quote! {
                if let Some(handler) = ::prism::events::ChangeHandler::find::<Self>(#field_name) {
                    let next = (handler.listen)(#reference);
                    listeners.push(Box::pin(async move {(vec![], next.await)}));
                } else {#children}
            });
            updates.push(quote! {
                if let Some(handler) = ::prism::events::ChangeHandler::find::<Self>(#field_name) {
                    if index == selected {
                        assert!(path.is_empty());
                        (handler.call)(self, ctx, update);
                        return
                    }
                    index += 1;
                } else {#child_updates}
            });
        }
        Ok((
            quote! {
                let mut listeners: Vec<::prism::events::ListenFuture<'_>> = Vec::new();
                #(#waits)*
                Box::pin(::prism::events::ChangeHandler::select(listeners))
            },
            quote! {
                let selected = path.pop().unwrap();
                let mut index = 0;
                #(#updates)*
                unreachable!("listener path no longer matches the component")
            },
        ))
    }

    fn sources(fields: &Fields) -> proc_macro2::TokenStream {
        let sources = fields.iter().enumerate().skip(1).map(|(index, field)| {
            let member = field.ident.clone().map(Member::Named)
                .unwrap_or(Member::Unnamed(Index::from(index)));
            let name = quote!(#member).to_string().replace("r#", "");
            let helper = format_ident!("__prism_source_{}", name);
            let field_type = &field.ty;
            quote! {
                fn #helper(source: &mut dyn ::std::any::Any) -> &mut #field_type {
                    source.downcast_mut::<#field_type>().unwrap()
                }
            }
        });
        quote! {#(#sources)*}
    }

    fn source(input: proc_macro2::TokenStream, component: &Type, field: &mut Option<Member>) -> syn::Result<proc_macro2::TokenStream> {
        let mut input = input.into_iter();
        let mut output = proc_macro2::TokenStream::new();
        while let Some(token) = input.next() {
            match token {
                proc_macro2::TokenTree::Ident(name) if name == "self" => {
                    let Some(proc_macro2::TokenTree::Punct(dot)) = input.next() else {
                        return Err(Error::new_spanned(name, "listen through one source field"))
                    };
                    if dot.as_char() != '.' {
                        return Err(Error::new_spanned(dot, "listen through one source field"))
                    }
                    let Some(token) = input.next() else {
                        return Err(Error::new_spanned(name, "expected a source field after self"))
                    };
                    let member = syn::parse2::<Member>(token.into())?;
                    if let Some(previous) = field {
                        if quote!(#previous).to_string().replace("r#", "")
                            != quote!(#member).to_string().replace("r#", "") {
                            return Err(Error::new_spanned(
                                member, "a listener can borrow one source field"
                            ))
                        }
                    } else {*field = Some(member.clone());}
                    let name = quote!(#member).to_string().replace("r#", "");
                    let helper = format_ident!("__prism_source_{}", name);
                    output.extend(quote! {(*<#component>::#helper(__prism_source))});
                }
                proc_macro2::TokenTree::Group(group) => {
                    let contents = Self::source(group.stream(), component, field)?;
                    let mut replacement = proc_macro2::Group::new(group.delimiter(), contents);
                    replacement.set_span(group.span());
                    output.extend([proc_macro2::TokenTree::Group(replacement)]);
                }
                token => output.extend([token]),
            }
        }
        Ok(output)
    }
}

struct Registration;
impl Registration {
    fn expand(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
        if !args.is_empty() {
            return Error::new_spanned(
                proc_macro2::TokenStream::from(args), "#[on_event] does not accept arguments"
            ).to_compile_error().into();
        }
        let implementation = parse_macro_input!(input as ItemImpl);
        let Some((negative, path, _)) = &implementation.trait_ else {
            return Error::new_spanned(&implementation, "#[on_event] requires impl OnEvent<T> for C")
                .to_compile_error().into();
        };
        let handler = path.segments.last().unwrap();
        if handler.ident != "OnEvent" {
            return Error::new_spanned(path, "#[on_event] requires impl OnEvent<T> for C")
                .to_compile_error().into();
        }
        if negative.is_some() || implementation.unsafety.is_some() {
            return Error::new_spanned(&implementation, "#[on_event] requires a safe, positive impl")
                .to_compile_error().into();
        }
        if !implementation.generics.params.is_empty() {
            return Error::new_spanned(
                &implementation.generics, "#[on_event] requires concrete component and event types"
            ).to_compile_error().into();
        }
        let PathArguments::AngleBracketed(arguments) = &handler.arguments else {
            return Error::new_spanned(path, "OnEvent requires exactly one type argument")
                .to_compile_error().into();
        };
        let Some(GenericArgument::Type(event)) = arguments.args.first()
            .filter(|_| arguments.args.len() == 1) else {
            return Error::new_spanned(arguments, "OnEvent requires exactly one type argument")
                .to_compile_error().into();
        };
        if let Err(error) = Self::concrete(event) {return error.to_compile_error().into()}
        let component = &implementation.self_ty;
        let conditions: Vec<_> = implementation.attrs.iter().filter(|attribute| {
            attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
        }).collect();

        let registration = quote! {
            #(#conditions)*
            impl ::prism::events::EventRegistration<#event> for #component {}
            #(#conditions)*
            ::prism::events::inventory::submit! {
                ::prism::events::EventHandler {
                    types: || {
                        (::std::any::TypeId::of::<#component>(), ::std::any::TypeId::of::<#event>())
                    },
                    call: |component, ctx, event| {
                        let component = component.downcast_mut::<#component>()
                            .expect("registered component type");
                        let event = event.downcast::<#event>().expect("registered event type");
                        <#component as ::prism::events::OnEvent<#event>>::on(component, ctx, *event)
                    },
                }
            }
        };
        quote! {#implementation #registration}.into()
    }

    fn change(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
        let expression = parse_macro_input!(args as syn::Expr);
        let mut implementation = parse_macro_input!(input as ItemImpl);
        if !implementation.generics.params.is_empty() {
            return Error::new_spanned(
                &implementation.generics, "#[on_change] requires a concrete component and update"
            ).to_compile_error().into();
        }
        let component = implementation.self_ty.clone();
        let Some((negative, path, _)) = &mut implementation.trait_ else {
            return Error::new_spanned(
                &implementation, "#[on_change(source)] requires impl OnChange<U> for C"
            ).to_compile_error().into();
        };
        if negative.is_some() || implementation.unsafety.is_some() {
            return Error::new_spanned(
                &implementation, "#[on_change] requires a safe, positive impl"
            ).to_compile_error().into();
        }
        let handler = path.segments.last_mut().unwrap();
        if handler.ident != "OnChange" {
            return Error::new_spanned(path, "#[on_change(source)] requires impl OnChange<U> for C")
                .to_compile_error().into();
        }
        let PathArguments::AngleBracketed(arguments) = &mut handler.arguments else {
            return Error::new_spanned(path, "OnChange requires exactly one update type")
                .to_compile_error().into();
        };
        let Some(GenericArgument::Type(update)) = arguments.args.first()
            .filter(|_| arguments.args.len() == 1) else {
            return Error::new_spanned(arguments, "OnChange requires exactly one update type")
                .to_compile_error().into();
        };
        let update = update.clone();
        if let Err(error) = Self::concrete(&update) {return error.to_compile_error().into()}
        let mut field = None;
        let source = match ChangeFields::source(quote!(#expression), &component, &mut field) {
            Ok(source) => source,
            Err(error) => return error.to_compile_error().into(),
        };
        let Some(field) = field else {
            return Error::new_spanned(expression, "a listener must borrow one source field")
                .to_compile_error().into();
        };
        let field_name = quote!(#field).to_string().replace("r#", "");
        let mut hasher = DefaultHasher::new();
        quote!(#component).to_string().hash(&mut hasher);
        field_name.hash(&mut hasher);
        let tag = format_ident!("__PrismChange{:x}", hasher.finish());
        arguments.args.push(syn::parse_quote!(#tag));
        let path = path.clone();
        let conditions: Vec<_> = implementation.attrs.iter().filter(|attribute| {
            attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
        }).collect();
        quote! {
            #(#conditions)*
            struct #tag;
            #implementation
            #(#conditions)*
            ::prism::events::inventory::submit! {
                ::prism::events::ChangeHandler {
                    component: || ::std::any::TypeId::of::<#component>(),
                    field: #field_name,
                    listen: |__prism_source| Box::pin(async move {
                        let update: #update = (#source).await;
                        Box::new(update) as Box<dyn ::std::any::Any>
                    }),
                    call: |component, ctx, update| {
                        let component = component.downcast_mut::<#component>().unwrap();
                        let update = *update.downcast::<#update>().unwrap();
                        <#component as #path>::on_change(component, ctx, update);
                    },
                }
            }
        }.into()
    }

    fn concrete(value: &Type) -> syn::Result<()> {
        let mut pending = vec![quote!(#value)];
        while let Some(tokens) = pending.pop() {
            for token in tokens {
                match token {
                    proc_macro2::TokenTree::Ident(name) if name == "Self" => {
                        return Err(Error::new_spanned(
                            value, "use the concrete component type, not Self"
                        ))
                    }
                    proc_macro2::TokenTree::Group(group) => pending.push(group.stream()),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
