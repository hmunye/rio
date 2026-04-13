#![allow(unused_macros)]

macro_rules! cfg_macros {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "macros")]
            #[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
            #[doc(inline)]
            $item
        )*
    }
}

macro_rules! cfg_not_macros {
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "macros"))]
            $item
        )*
    }
}

macro_rules! cfg_time {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "time")]
            #[cfg_attr(docsrs, doc(cfg(feature = "time")))]
            $item
        )*
    }
}

macro_rules! cfg_not_time {
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "time"))]
            $item
        )*
    }
}

macro_rules! cfg_io {
    ($($item:item)*) => {
        #[cfg(all(feature = "io", not(target_os = "linux")))]
        compile_error!(
            "`io` feature requires a target with epoll support (Linux)."
        );

        $(
            #[cfg(feature = "io")]
            #[cfg_attr(docsrs, doc(cfg(feature = "io")))]
            $item
        )*
    }
}

macro_rules! cfg_not_io {
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "io"))]
            $item
        )*
    }
}

macro_rules! cfg_net {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "net")]
            #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
            $item
        )*
    }
}

macro_rules! cfg_not_net {
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "net"))]
            $item
        )*
    }
}

macro_rules! cfg_test {
    ($($item:item)*) => {
        $(
            #[cfg(test)]
            $item
        )*
    }
}

macro_rules! cfg_not_test {
    ($($item:item)*) => {
        $(
            #[cfg(not(test))]
            $item
        )*
    }
}

macro_rules! rt {
    ($($tt:tt)*) => {
        let rt = crate::rt::Runtime::new();
        rt.block_on(async {
            $(
                $tt
            )*
        })
    };
}

macro_rules! errno {
    ($($tt:tt)+) => {{
        let errno = ::std::io::Error::last_os_error();
        let prefix = format!($($tt)+);
        ::std::io::Error::new(errno.kind(), format!("{prefix}: {errno}"))
    }};
}
