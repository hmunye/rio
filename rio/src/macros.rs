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

/// Helper macro used to execute asynchronous code within a synchronous runtime
/// entry point, specifically for unit tests.
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
