use proc_macro2::Span;
use proc_macro_error::proc_macro_error;
use syn::{
    parse, parse_macro_input, spanned::Spanned, token, Abi, Expr, Ident, Item, ItemFn, LitStr,
    ReturnType, Type, Visibility,
};

use proc_macro::TokenStream;
use quote::{quote, ToTokens};

#[proc_macro_attribute]
pub fn entry(args: TokenStream, input: TokenStream) -> TokenStream {
    let f = parse_macro_input!(input as ItemFn);

    // check the function arguments
    if !f.sig.inputs.is_empty() {
        return parse::Error::new(
            f.sig.inputs.last().unwrap().span(),
            "`#[entry]` function accepts no arguments",
        )
        .to_compile_error()
        .into();
    }

    // check the function signature
    let valid_signature = f.sig.constness.is_none()
        && f.sig.asyncness.is_none()
        && f.vis == Visibility::Inherited
        && f.sig.abi.is_none()
        && f.sig.generics.params.is_empty()
        && f.sig.generics.where_clause.is_none()
        && f.sig.variadic.is_none()
        && match f.sig.output {
            ReturnType::Default => false,
            ReturnType::Type(_, ref ty) => matches!(**ty, Type::Never(_)),
        };

    if !valid_signature {
        return parse::Error::new(
            f.span(),
            "`#[entry]` function must have signature `[unsafe] fn() -> !`",
        )
        .to_compile_error()
        .into();
    }

    if !args.is_empty() {
        return parse::Error::new(Span::call_site(), "This attribute accepts no arguments")
            .to_compile_error()
            .into();
    }

    // XXX should we blacklist other attributes?
    let attrs = f.attrs;
    let unsafety = f.sig.unsafety;
    let args = f.sig.inputs;
    let stmts = f.block.stmts;

    quote!(
        #[allow(non_snake_case)]
        #[export_name = "main"]
        #(#attrs)*
        pub #unsafety fn __hpm_riscv_v_rt__main(#args) -> ! {
            #(#stmts)*
        }
    )
    .into()
}

/// This attribute allows placing functions into ram.
#[proc_macro_attribute]
#[proc_macro_error]
pub fn fast(_args: TokenStream, input: TokenStream) -> TokenStream {
    let f = parse_macro_input!(input as Item);

    match f {
        Item::Fn(f) => {
            let section = quote! {
                #[link_section = ".fast.text"]
                #[inline(never)] // make certain function is not inlined
            };

            quote!(
                #section
                #f
            )
            .into()
        }
        Item::Static(item) => {
            let mut section = quote! {
                #[link_section = ".fast.data"]
            };

            if let Expr::Call(c) = &*item.expr {
                let s = format!("{}", c.into_token_stream());

                if s.ends_with("MaybeUninit :: uninit()")
                    || s.ends_with("MaybeUninit :: uninit_array()")
                {
                    section = quote! {
                        #[link_section = ".fast.bss"]
                    };
                }
            }

            quote!(
                #section
                #item
            )
            .into()
        }
        _ => {
            let msg = "expected function or static";
            let span = f.span();
            return syn::Error::new(span, msg).to_compile_error().into();
        }
    }
}

const CORE_INTERRUPTS: [&str; 6] = [
    "SupervisorSoft",
    "MachineSoft",
    "SupervisorTimer",
    "MachineTimer",
    "SupervisorExternal",
    "MachineExternal",
];

/// Marks a function as an interrupt handler. (Wrapping as a mret function)
///
/// Note that Rust has also introduced the `riscv-interrupt-m` and `riscv-interrupt-s` ABI, which
/// are used for machine and supervisor mode interrupts, respectively. These ABIs can also be used for
/// Qingke cores, yet they add additional register saving and restoring that is not necessary.
///
/// Usage:
/// ```ignore
/// #[interrupt]
/// fn UART0() { ... }
///
/// #[interrupt(MachineTimer)]
/// fn SysTick() { ... }
/// ```
#[proc_macro_attribute]
pub fn interrupt(args: TokenStream, input: TokenStream) -> TokenStream {
    use syn::{AttributeArgs, Meta, NestedMeta};

    let mut f = parse_macro_input!(input as ItemFn);

    let mut link_name = f.sig.ident.to_string();
    let mut is_core_irq = false;

    if !args.is_empty() {
        let args: AttributeArgs = parse_macro_input!(args as AttributeArgs);
        if args.len() > 1 {
            return parse::Error::new(
                Span::call_site(),
                "Accept form: #[interrupt], #[interrupt(InterruptName)], #[interrupt(InterruptName)]",
            )
            .to_compile_error()
            .into();
        }

        if let NestedMeta::Meta(Meta::Path(ref p)) = args[0] {
            if let Some(ident) = p.get_ident() {
                if let Some(irq_name) = CORE_INTERRUPTS.iter().find(|s| ident == *s) {
                    link_name = irq_name.to_string();
                    is_core_irq = true;
                } else {
                    link_name = ident.to_string();
                }
            }
        } else {
            return parse::Error::new(
                Span::call_site(),
                "Wrong type of argument, expected a core interrupt name",
            )
            .to_compile_error()
            .into();
        }
    } else {
        if CORE_INTERRUPTS.iter().any(|s| link_name == *s) {
            is_core_irq = true;
        }
    }

    // check the function arguments
    if !f.sig.inputs.is_empty() {
        return parse::Error::new(
            f.sig.inputs.last().unwrap().span(),
            "`#[interrupt]` function accepts no arguments",
        )
        .to_compile_error()
        .into();
    }

    let valid_signature = f.sig.constness.is_none()
        && f.vis == Visibility::Inherited
        && f.sig.abi.is_none()
        && f.sig.generics.params.is_empty()
        && f.sig.generics.where_clause.is_none()
        && f.sig.variadic.is_none()
        && match f.sig.output {
            ReturnType::Default => true,
            ReturnType::Type(_, ref ty) => match **ty {
                Type::Tuple(ref tuple) => tuple.elems.is_empty(),
                Type::Never(..) => true,
                _ => false,
            },
        }
        && f.sig.inputs.len() <= 1;

    if !valid_signature {
        return parse::Error::new(
            f.span(),
            "`#[interrupt]` handlers must have signature `[unsafe] fn() [-> !]`",
        )
        .to_compile_error()
        .into();
    }

    if !is_core_irq {
        f.sig.abi = Some(Abi {
            extern_token: token::Extern(Span::call_site()),
            name: Some(LitStr::new("riscv-interrupt-m", Span::call_site())),
        });
        f.sig.unsafety = Some(token::Unsafe(Span::call_site()))
    } else {
        f.sig.abi = Some(Abi {
            extern_token: token::Extern(Span::call_site()),
            name: Some(LitStr::new("C", Span::call_site())),
        });
    }

    f.sig.ident = Ident::new(
        &format!("__hpm_riscv_rt_isr_{}", link_name),
        Span::call_site(),
    );

    quote!(
        #[allow(non_snake_case)]
        #[link_section = ".isr_vector"]
        #[link_name = #link_name]
        #f
    )
    .into()
}

#[proc_macro_attribute]
pub fn pre_init(args: TokenStream, input: TokenStream) -> TokenStream {
    let f = parse_macro_input!(input as ItemFn);

    // check the function signature
    let valid_signature = f.sig.constness.is_none()
        && f.vis == Visibility::Inherited
        && f.sig.unsafety.is_some()
        && f.sig.abi.is_none()
        && f.sig.inputs.is_empty()
        && f.sig.generics.params.is_empty()
        && f.sig.generics.where_clause.is_none()
        && f.sig.variadic.is_none()
        && match f.sig.output {
            ReturnType::Default => true,
            ReturnType::Type(_, ref ty) => match **ty {
                Type::Tuple(ref tuple) => tuple.elems.is_empty(),
                _ => false,
            },
        };

    if !valid_signature {
        return parse::Error::new(
            f.span(),
            "`#[pre_init]` function must have signature `unsafe fn()`",
        )
        .to_compile_error()
        .into();
    }

    if !args.is_empty() {
        return parse::Error::new(Span::call_site(), "This attribute accepts no arguments")
            .to_compile_error()
            .into();
    }

    // XXX should we blacklist other attributes?
    let attrs = f.attrs;
    let ident = f.sig.ident;
    let block = f.block;

    quote!(
        #[export_name = "__pre_init"]
        #[allow(missing_docs)]  // we make a private fn public, which can trigger this lint
        #(#attrs)*
        pub unsafe fn #ident() #block
    )
    .into()
}
