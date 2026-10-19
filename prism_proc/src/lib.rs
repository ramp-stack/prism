use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericArgument, Index, ItemImpl, Member, PathArguments};
use syn::{Error, parse_macro_input};

#[proc_macro_derive(Component, attributes(skip, on_dyn_event))]
pub fn derive_component(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut dynamic = false;
    for attribute in &input.attrs {
        if !attribute.path().is_ident("on_dyn_event") {continue;}
        if !matches!(&attribute.meta, syn::Meta::Path(_)) {
            return Error::new_spanned(attribute, "use #[on_dyn_event] without arguments")
                .to_compile_error().into();
        }
        if dynamic {
            return Error::new_spanned(attribute, "duplicate #[on_dyn_event]")
                .to_compile_error().into();
        }
        dynamic = true;
    }

    let (children, children_mut, layout) = match &input.data {
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
            )
        }
        Data::Enum(data) => {
            let mut children_arms = Vec::new();
            let mut children_mut_arms = Vec::new();
            let mut layout_arms = Vec::new();

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
            )
        }
        Data::Union(_) => panic!("Cannot implement Component for a union"),
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
            #on_event
        }
    }.into()
}

#[proc_macro_attribute]
pub fn on_event(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if !args.is_empty() {
        return Error::new_spanned(
            proc_macro2::TokenStream::from(args), "#[on_event] does not accept arguments"
        ).to_compile_error().into();
    }
    let implementation = parse_macro_input!(input as ItemImpl);
    let Some((negative, path, _)) = &implementation.trait_ else {
        return Error::new_spanned(&implementation, "#[on_event] requires impl OnEvent<E> for C")
            .to_compile_error().into();
    };
    let handler = path.segments.last().unwrap();
    if handler.ident != "OnEvent" {
        return Error::new_spanned(path, "#[on_event] requires impl OnEvent<E> for C")
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
        return Error::new_spanned(path, "OnEvent requires exactly one event type")
            .to_compile_error().into();
    };
    let Some(GenericArgument::Type(event)) = arguments.args.first()
        .filter(|_| arguments.args.len() == 1) else {
        return Error::new_spanned(arguments, "OnEvent requires exactly one event type")
            .to_compile_error().into();
    };
    let mut pending = vec![quote!(#event)];
    while let Some(tokens) = pending.pop() {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Ident(name) if name == "Self" => {
                    return Error::new_spanned(event, "Use the concrete component type instead of Self")
                        .to_compile_error().into();
                }
                proc_macro2::TokenTree::Group(group) => pending.push(group.stream()),
                _ => {}
            }
        }
    }
    let component = &implementation.self_ty;
    let conditions: Vec<_> = implementation.attrs.iter().filter(|attribute| {
        attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
    }).collect();

    quote! {
        #implementation
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
    }.into()
}
