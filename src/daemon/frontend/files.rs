use futures::future::{ok, FutureResult};
use tk_http::{Status};
use tk_http::server::{Error, Encoder, EncoderDone};
use frontend::{reply, Request};


#[derive(Debug)]
pub struct File {
    pub data: &'static [u8],
    pub content_type: &'static str,
}

pub static INDEX: File = File {
    data: include_bytes!("../../../public/index.html"),
    content_type: "text/html; charset=utf-8",
};

pub static MAIN_CSS: File = File {
    data: include_bytes!("../../../public/css/main.css"),
    content_type: "text/css; charset=utf-8",
};

pub static BOOTSTRAP_CSS: File = File {
    data: include_bytes!("../../../public/css/bootstrap.min.css"),
    content_type: "text/css; charset=utf-8",
};

pub static BUNDLE_JS: File = File {
    data: include_bytes!("../../../public/js/bundle.js"),
    content_type: "application/javascript; charset=utf-8",
};


impl File {
    pub fn serve<S: 'static>(&'static self) -> Result<Request<S>, Error> {
        Ok(reply(move |e| Box::new(serve_file(self, e))))
    }
}

pub fn serve_file<S: 'static>(file: &File, mut e: Encoder<S>)
    -> FutureResult<EncoderDone<S>, Error>
{
    e.status(Status::Ok);
    e.add_header("Content-Type", file.content_type).unwrap();
    e.add_length(file.data.len() as u64).unwrap();
    if e.done_headers().unwrap() {
        e.write_body(file.data);
    }
    ok(e.done())
}
