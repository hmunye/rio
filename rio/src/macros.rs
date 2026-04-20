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
        #[cfg(all(feature = "io", not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))))]
        compile_error!(
            "io feature requires a target with either epoll (Linux) or kqueue (macOS/BSD) support."
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

macro_rules! cfg_linux {
    ($($item:item)*) => {
        $(
            #[cfg(target_os = "linux")]
            $item
        )*
    }
}

macro_rules! cfg_bsd {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                target_os = "macos",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd"
            ))]
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
