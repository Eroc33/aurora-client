error_chain! {
    foreign_links {
        Io(::std::io::Error);
        Hyper(::hyper::Error);
    }
    errors{
        UploadFailed{
            description("Failed to upload an inverter reading")
        }
    }
}
