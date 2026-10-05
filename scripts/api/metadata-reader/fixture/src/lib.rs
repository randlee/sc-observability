//! Compiled as part of the normal unit build, never compiled by the API reader.
macro_rules! specimen {
    ($module:ident, $field:ty, $arg:ty, $bound:path) => {
        pub mod $module {
            pub struct Value {
                pub field: $field,
            }
            impl Value {
                pub fn method(&self, input: $arg) -> $arg {
                    input
                }
            }
            pub trait Contract: $bound {
                fn read(&self) -> u32;
            }
            pub use Value as Alias;
        }
    };
}
specimen!(baseline, u32, u32, Send);
specimen!(field, u64, u32, Send);
specimen!(signature, u32, u64, Send);
specimen!(trait_bound, u32, u32, Sync);
pub mod visibility {
    pub struct Value {
        pub field: u32,
    }
    impl Value {
        pub fn method(&self, input: u32) -> u32 {
            input
        }
    }
    pub trait Contract: Send {
        fn read(&self) -> u32;
    }
    pub(crate) use Value as Alias;
    fn _use_alias(_: Alias) {}
}
pub mod auto_trait {
    pub struct Value {
        pub field: u32,
        _private: std::rc::Rc<()>,
    }
}
pub enum Error {
    First,
    Second(u32),
}

mod private_trait {
    pub trait NotExported {}
    impl<T> NotExported for T {}
    pub fn touch<T: NotExported>() {}
}
#[doc(hidden)]
pub fn use_private_trait() {
    private_trait::touch::<u32>();
}
