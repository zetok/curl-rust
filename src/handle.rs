use super::ffi::easy::Easy;
use super::body::{Body,ToBody};
use super::ffi;
use super::ffi::opt;
use {ProgressCb,ErrCode,Response};

static DEFAULT_TIMEOUT_MS: uint = 30_000;

pub struct Handle {
  easy: Easy,
}

impl Handle {
  pub fn new() -> Handle {
    Handle { easy: Easy::new() }
      .timeout(DEFAULT_TIMEOUT_MS)
      .connect_timeout(DEFAULT_TIMEOUT_MS)
  }

  pub fn timeout(mut self, ms: uint) -> Handle {
    self.easy.setopt(opt::TIMEOUT_MS, ms).unwrap();
    self
  }

  pub fn connect_timeout(mut self, ms: uint) -> Handle {
    self.easy.setopt(opt::CONNECTTIMEOUT_MS, ms).unwrap();
    self
  }

  pub fn get<'a, 'b>(&'a mut self, uri: &str) -> Request<'a, 'b> {
    Request::new(self, Get, None).uri(uri)
  }

  pub fn head<'a, 'b>(&'a mut self, uri: &str) -> Request<'a, 'b> {
    Request::new(self, Head, None).uri(uri)
  }

  pub fn post<'a, 'b, B: ToBody<'b>>(&'a mut self, uri: &str, body: B) -> Request<'a, 'b> {
    Request::new(self, Post, Some(body.to_body())).uri(uri)
  }
}

pub enum Method {
  Options,
  Get,
  Head,
  Post,
  Put,
  Delete,
  Trace,
  Connect
}

pub struct Request<'a, 'b> {
  err: Option<ErrCode>,
  handle: &'a mut Handle,
  headers: ffi::List,
  body: Option<Body<'b>>,
  body_type: bool, // whether or not the body type was set
  content_type: bool, // whether or not the content type was set
  expect_continue: bool, // whether to expect a 100 continue from the server
  progress: Option<ProgressCb<'b>>,
}

impl<'a, 'b> Request<'a, 'b> {
  fn new<'a, 'b>(handle: &'a mut Handle, method: Method, body: Option<Body<'b>>) -> Request<'a, 'b> {
    macro_rules! set_method(
      ($val:expr) => ({
        match handle.easy.setopt($val, 1i) {
          Ok(_) => None,
          Err(e) => Some(e)
        }
      });
    )

    // TODO: track errors
    let err = match method {
      Get => set_method!(opt::HTTPGET),
      Head => set_method!(opt::NOBODY),
      Post => set_method!(opt::POST),
      _ => { unimplemented!() }
    };

    Request {
      err: err,
      handle: handle,
      headers: ffi::List::new(),
      body: body,
      body_type: false,
      content_type: false,
      expect_continue: false,
      progress: None
    }
  }

  pub fn uri(mut self, uri: &str) -> Request<'a, 'b> {
    match self.handle.easy.setopt(opt::URL, uri) {
      Ok(_) => {}
      Err(e) => self.err = Some(e)
    }

    self
  }

  pub fn content_length(mut self, len: uint) -> Request<'a, 'b> {
    if !self.body_type {
      self.body_type = true;
      append_header(&mut self.headers, "Content-Type", len.to_str().as_slice());
    }

    self
  }

  pub fn chunked(mut self) -> Request<'a, 'b> {
    if !self.body_type {
      self.body_type = true;
      append_header(&mut self.headers, "Transfer-Encoding", "chunked");
    }

    self
  }

  pub fn expect_continue(mut self) -> Request<'a, 'b> {
    self.expect_continue = true;
    self
  }

  pub fn header(mut self, name: &str, val: &str) -> Request<'a, 'b> {
    append_header(&mut self.headers, name, val);
    self
  }

  pub fn headers<'c, I: Iterator<(&'c str, &'c str)>>(mut self, mut hdrs: I) -> Request<'a, 'b> {
    for (name, val) in hdrs {
      append_header(&mut self.headers, name, val);
    }

    self
  }

  pub fn progress(mut self, cb: ProgressCb<'b>) -> Request<'a, 'b> {
    self.progress = Some(cb);
    self
  }

  pub fn exec(self) -> Result<Response, ErrCode> {
    let Request { err, handle, mut headers, mut body, body_type, content_type, expect_continue, progress, .. } = self;

    match err {
      Some(e) => return Err(e),
      None => {}
    }

    // Clear custom headers set from the previous request
    try!(handle.easy.setopt(opt::HTTPHEADER, 0u));

    match body.as_ref() {
      None => {}
      Some(body) => {
        if !body_type {
          match body.get_size() {
            Some(len) => append_header(&mut headers, "Content-Length", len.to_str().as_slice()),
            None => append_header(&mut headers, "Transfer-Encoding", "chunked")
          }
        }

        if !content_type {
          append_header(&mut headers, "Content-Type", "application/octet-stream");
        }

        if !expect_continue {
          append_header(&mut headers, "Expect", "");
        }
      }
    }

    if !headers.is_empty() {
      try!(handle.easy.setopt(opt::HTTPHEADER, &headers));
    }

    handle.easy.perform(body.as_mut(), progress)
  }
}

fn append_header(list: &mut ffi::List, name: &str, val: &str) {
  let mut c_str = Vec::with_capacity(name.len() + val.len() + 3);
  c_str.push_all(name.as_bytes());
  c_str.push(':' as u8);
  c_str.push(' ' as u8);
  c_str.push_all(val.as_bytes());
  c_str.push(0);

  list.push_bytes(c_str.as_slice());
}
