error_chain! {
    foreign_links {
        Io(::std::io::Error);
        Hyper(::hyper::Error);
        TimerError(::tokio_timer::TimerError);
    }
    errors{
    }
}