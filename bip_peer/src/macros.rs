#[macro_use]
macro_rules! throwaway_input (
    ($res:expr) => (
        {
            match $res {
                IResult::Done(_, result) => IResult::Done((), result),
                IResult::Error(e)        => IResult::Error(e),
                IResult::Incomplete(i)   => IResult::Incomplete(i)
            }
        }
    );
    ($i:expr, $func:path) => (
        {
            throwaway_input!($func($i))
        }
    );
    ($i:expr, $submac:ident!( $($args:tt)* )) => (
        {
            throwaway_input!($submac!($i, $($args)*))
        }
    );
);

#[macro_use]
macro_rules! ignore_input (
    ($i:expr, $submac:ident!( $($args:tt)* )) => (
        {
            $submac!($($args)*)
        }
    );
);
