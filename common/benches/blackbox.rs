//run: cargo bench

#![feature(test)]

macro_rules! declare {
    ($enum:ident, $( $variant:ident $(, $val:expr)*; )*) => {
        #[derive(Debug)]
        #[repr(usize)]
        pub enum $enum {
            $( $variant, )*
        }
        impl $enum {
            pub const fn id(&self) -> usize {
                // SAFETY: The representation of $enum is set
                unsafe { *(self as *const Self as *const usize) }
            }
        }

        const VTABLE: [&dyn ConstDispatch; 0 $( + { _ = $enum::$variant; 1 } )*] = [
            $( &$variant(), )*
        ];


        pub trait EnumDispatch {
            fn return_ed<'a>(&self, source: &'a str) -> usize;
        }
        impl EnumDispatch for $enum {
            fn return_ed<'a>(&self, source: &'a str) -> usize {
                match self {
                    $( $enum::$variant => $variant().return_ed(source), )*
                }
            }
        }

        pub trait ConstDispatch {
            fn return_cd<'a>(&self, source: &'a str) -> usize;
        }

        impl ConstDispatch for $enum {
            fn return_cd<'a>(&self, source: &'a str) -> usize {
                VTABLE[self.id()].return_cd(source)
            }
        }
    };
}

declare! {Id,
    AsciiDoctor;
    CommonMark;
}

struct AsciiDoctor();
impl EnumDispatch for AsciiDoctor {
    fn return_ed<'a>(&self, source: &'a str) -> usize {
        source.len() + 1
    }
}
impl ConstDispatch for AsciiDoctor {
    fn return_cd<'a>(&self, source: &'a str) -> usize {
        source.len() + 1
    }
}

struct CommonMark();
impl EnumDispatch for CommonMark {
    fn return_ed<'a>(&self, source: &'a str) -> usize {
        source.len() + 2
    }
}
impl ConstDispatch for CommonMark {
    fn return_cd<'a>(&self, source: &'a str) -> usize {
        source.len() + 2
    }
}



#[cfg(test)]
mod benches {
    use super::*;
    extern crate test;
    use test::{Bencher, black_box};
    const ITERATIONS: usize = 1000000;

    // The VTABLE solution is about 5 times faster
    //test benches::const_dispatch ... bench:   3,755,605 ns/iter (+/- 352,153)
    //test benches::enum_dispatch  ... bench:     771,933 ns/iter (+/- 104,112)
    #[bench]
    fn enum_dispatch(b: &mut Bencher) {
        let one = Id::AsciiDoctor;
        let two = Id::CommonMark;

        b.iter(|| {
            for _ in 0..ITERATIONS {
                black_box(one.return_ed("hello"));
                black_box(two.return_ed("hello"));
            }
        })
    }

    #[bench]
    fn const_dispatch(b: &mut Bencher) {
        let one = Id::AsciiDoctor;
        let two = Id::CommonMark;

        b.iter(|| {
            for _ in 0..ITERATIONS {
                black_box(one.return_cd("hello"));
                black_box(two.return_cd("hello"));
            }
        })
    }
}
