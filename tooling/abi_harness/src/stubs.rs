//! Host stubs for the WASI interfaces the `http!` effect reaches.
//!
//! Generated from this crate's `bindgen!` expansion (`cargo +nightly rustc --
//! -Zunpretty=expanded`) and then hand-tuned: every function is a stub except
//! the handful the harness actually drives, which are marked below. Keeping the
//! stubs here rather than in `main.rs` keeps the checks readable.
//!
//! wasmtime's own `wasi:http` implementation tracks a newer WASI version
//! (`wasi:http@0.2.12`) than the one Golem's WIT bundle pins (`@0.2.3`), so the
//! harness implements the interfaces itself instead of linking that crate.
#![allow(unused_variables, clippy::all)]

use crate::Ctx;


pub mod stubs_wasi_http_outgoing_handler {
    use super::Ctx;
    use crate::wasi::http::outgoing_handler::*;

    impl Host for Ctx {
        fn handle(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>, _p2: Option<wasmtime::component::Resource<RequestOptions>>) -> Result<wasmtime::component::Resource<FutureIncomingResponse>, ErrorCode> {
            let request = self
            .requests_table
            .get(_p1.rep() as usize)
            .cloned()
            .unwrap_or_default();
        self.http_request_lines.push(format!(
            "{} {} host={}",
            request.method, request.path, request.authority
        ));
        self.request_body.clear();
        self.response_body = b"{\"stub\":\"http\"}".to_vec();
        self.response_cursor = 0;
        let rep = self.body_slots.len() as u32;
        self.body_slots.push(());
        Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
        }
    }
}

pub mod stubs_wasi_http_types {
    use super::Ctx;
    use crate::method_name;
    use crate::StubRequest;
    use crate::wasi::http::types::*;

    impl HostFields for Ctx {
        fn new(&mut self) -> wasmtime::component::Resource<Fields> {
            let rep = self.fields_table.len() as u32;
            self.fields_table.push(std::collections::HashMap::new());
            unsafe { wasmtime::component::Resource::new_own(rep) }
        }
        fn from_list(&mut self, _p1: wasmtime::component::__internal::Vec<(FieldName, FieldValue)>) -> Result<wasmtime::component::Resource<Fields>, HeaderError> {
            unimplemented!("harness stub: wasi::http::types::HostFields::from_list")
        }
        fn get(&mut self, _p1: wasmtime::component::Resource<Fields>, _p2: FieldName) -> wasmtime::component::__internal::Vec<FieldValue> {
            unimplemented!("harness stub: wasi::http::types::HostFields::get")
        }
        fn has(&mut self, _p1: wasmtime::component::Resource<Fields>, _p2: FieldName) -> bool {
            unimplemented!("harness stub: wasi::http::types::HostFields::has")
        }
        fn set(&mut self, _p1: wasmtime::component::Resource<Fields>, _p2: FieldName, _p3: wasmtime::component::__internal::Vec<FieldValue>) -> Result<(), HeaderError> {
            match self.fields_table.get_mut(_p1.rep() as usize) {
                Some(fields) => {
                    fields.insert(_p2, _p3);
                    Ok(())
                }
                None => Err(HeaderError::Immutable),
            }
        }
        fn delete(&mut self, _p1: wasmtime::component::Resource<Fields>, _p2: FieldName) -> Result<(), HeaderError> {
            unimplemented!("harness stub: wasi::http::types::HostFields::delete")
        }
        fn append(&mut self, _p1: wasmtime::component::Resource<Fields>, _p2: FieldName, _p3: FieldValue) -> Result<(), HeaderError> {
            unimplemented!("harness stub: wasi::http::types::HostFields::append")
        }
        fn entries(&mut self, _p1: wasmtime::component::Resource<Fields>) -> wasmtime::component::__internal::Vec<(FieldName, FieldValue)> {
            match self.fields_table.get(_p1.rep() as usize) {
                Some(fields) => fields
                    .iter()
                    .map(|(name, values)| {
                        (name.clone(), values.first().cloned().unwrap_or_default())
                    })
                    .collect(),
                None => Vec::new(),
            }
        }
        fn clone(&mut self, _p1: wasmtime::component::Resource<Fields>) -> wasmtime::component::Resource<Fields> {
            unimplemented!("harness stub: wasi::http::types::HostFields::clone")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<Fields>) -> wasmtime::Result<()> {
            Ok(())
        }
    }

    impl HostIncomingRequest for Ctx {
        fn method(&mut self, _p1: wasmtime::component::Resource<IncomingRequest>) -> Method {
            unimplemented!("harness stub: wasi::http::types::HostIncomingRequest::method")
        }
        fn path_with_query(&mut self, _p1: wasmtime::component::Resource<IncomingRequest>) -> Option<wasmtime::component::__internal::String> {
            unimplemented!("harness stub: wasi::http::types::HostIncomingRequest::path_with_query")
        }
        fn scheme(&mut self, _p1: wasmtime::component::Resource<IncomingRequest>) -> Option<Scheme> {
            unimplemented!("harness stub: wasi::http::types::HostIncomingRequest::scheme")
        }
        fn authority(&mut self, _p1: wasmtime::component::Resource<IncomingRequest>) -> Option<wasmtime::component::__internal::String> {
            unimplemented!("harness stub: wasi::http::types::HostIncomingRequest::authority")
        }
        fn headers(&mut self, _p1: wasmtime::component::Resource<IncomingRequest>) -> wasmtime::component::Resource<Headers> {
            unimplemented!("harness stub: wasi::http::types::HostIncomingRequest::headers")
        }
        fn consume(&mut self, _p1: wasmtime::component::Resource<IncomingRequest>) -> Result<wasmtime::component::Resource<IncomingBody>, ()> {
            unimplemented!("harness stub: wasi::http::types::HostIncomingRequest::consume")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<IncomingRequest>) -> wasmtime::Result<()> {
            unimplemented!("harness stub: wasi::http::types::HostIncomingRequest::drop")
        }
    }

    impl HostOutgoingRequest for Ctx {
        fn new(&mut self, _p1: wasmtime::component::Resource<Headers>) -> wasmtime::component::Resource<OutgoingRequest> {
            let rep = self.requests_table.len() as u32;
            self.requests_table.push(StubRequest::default());
            unsafe { wasmtime::component::Resource::new_own(rep) }
        }
        fn body(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>) -> Result<wasmtime::component::Resource<OutgoingBody>, ()> {
            let rep = self.body_slots.len() as u32;
            self.body_slots.push(());
            Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
        }
        fn method(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>) -> Method {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingRequest::method")
        }
        fn set_method(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>, _p2: Method) -> Result<(), ()> {
            if let Some(request) = self.requests_table.get_mut(_p1.rep() as usize) {
                request.method = method_name(&_p2);
                Ok(())
            } else {
                Err(())
            }
        }
        fn path_with_query(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>) -> Option<wasmtime::component::__internal::String> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingRequest::path_with_query")
        }
        fn set_path_with_query(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>, _p2: Option<wasmtime::component::__internal::String>) -> Result<(), ()> {
            if let Some(request) = self.requests_table.get_mut(_p1.rep() as usize) {
                request.path = _p2.unwrap_or_default();
                Ok(())
            } else {
                Err(())
            }
        }
        fn scheme(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>) -> Option<Scheme> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingRequest::scheme")
        }
        fn set_scheme(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>, _p2: Option<Scheme>) -> Result<(), ()> {
            if let Some(request) = self.requests_table.get_mut(_p1.rep() as usize) {
                request.scheme = match _p2 {
                    Some(Scheme::Http) => String::from("http"),
                    Some(Scheme::Https) => String::from("https"),
                    Some(Scheme::Other(other)) => other,
                    None => String::new(),
                };
                Ok(())
            } else {
                Err(())
            }
        }
        fn authority(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>) -> Option<wasmtime::component::__internal::String> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingRequest::authority")
        }
        fn set_authority(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>, _p2: Option<wasmtime::component::__internal::String>) -> Result<(), ()> {
            if let Some(request) = self.requests_table.get_mut(_p1.rep() as usize) {
                request.authority = _p2.unwrap_or_default();
                Ok(())
            } else {
                Err(())
            }
        }
        fn headers(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>) -> wasmtime::component::Resource<Headers> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingRequest::headers")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<OutgoingRequest>) -> wasmtime::Result<()> {
            Ok(())
        }
    }

    impl HostRequestOptions for Ctx {
        fn new(&mut self) -> wasmtime::component::Resource<RequestOptions> {
            unimplemented!("harness stub: wasi::http::types::HostRequestOptions::new")
        }
        fn connect_timeout(&mut self, _p1: wasmtime::component::Resource<RequestOptions>) -> Option<Duration> {
            unimplemented!("harness stub: wasi::http::types::HostRequestOptions::connect_timeout")
        }
        fn set_connect_timeout(&mut self, _p1: wasmtime::component::Resource<RequestOptions>, _p2: Option<Duration>) -> Result<(), ()> {
            unimplemented!("harness stub: wasi::http::types::HostRequestOptions::set_connect_timeout")
        }
        fn first_byte_timeout(&mut self, _p1: wasmtime::component::Resource<RequestOptions>) -> Option<Duration> {
            unimplemented!("harness stub: wasi::http::types::HostRequestOptions::first_byte_timeout")
        }
        fn set_first_byte_timeout(&mut self, _p1: wasmtime::component::Resource<RequestOptions>, _p2: Option<Duration>) -> Result<(), ()> {
            unimplemented!("harness stub: wasi::http::types::HostRequestOptions::set_first_byte_timeout")
        }
        fn between_bytes_timeout(&mut self, _p1: wasmtime::component::Resource<RequestOptions>) -> Option<Duration> {
            unimplemented!("harness stub: wasi::http::types::HostRequestOptions::between_bytes_timeout")
        }
        fn set_between_bytes_timeout(&mut self, _p1: wasmtime::component::Resource<RequestOptions>, _p2: Option<Duration>) -> Result<(), ()> {
            unimplemented!("harness stub: wasi::http::types::HostRequestOptions::set_between_bytes_timeout")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<RequestOptions>) -> wasmtime::Result<()> {
            Ok(())
        }
    }

    impl HostResponseOutparam for Ctx {
        fn set(&mut self, _p1: wasmtime::component::Resource<ResponseOutparam>, _p2: Result<wasmtime::component::Resource<OutgoingResponse>, ErrorCode>) -> () {
            unimplemented!("harness stub: wasi::http::types::HostResponseOutparam::set")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<ResponseOutparam>) -> wasmtime::Result<()> {
            unimplemented!("harness stub: wasi::http::types::HostResponseOutparam::drop")
        }
    }

    impl HostIncomingResponse for Ctx {
        fn status(&mut self, _p1: wasmtime::component::Resource<IncomingResponse>) -> StatusCode {
            200
        }
        fn headers(&mut self, _p1: wasmtime::component::Resource<IncomingResponse>) -> wasmtime::component::Resource<Headers> {
            let rep = self.fields_table.len() as u32;
            let mut fields = std::collections::HashMap::new();
            fields.insert(
                String::from("content-type"),
                vec![b"application/json".to_vec()],
            );
            self.fields_table.push(fields);
            unsafe { wasmtime::component::Resource::new_own(rep) }
        }
        fn consume(&mut self, _p1: wasmtime::component::Resource<IncomingResponse>) -> Result<wasmtime::component::Resource<IncomingBody>, ()> {
            let rep = self.body_slots.len() as u32;
            self.body_slots.push(());
            Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<IncomingResponse>) -> wasmtime::Result<()> {
            Ok(())
        }
    }

    impl HostIncomingBody for Ctx {
        fn stream(&mut self, _p1: wasmtime::component::Resource<IncomingBody>) -> Result<wasmtime::component::Resource<InputStream>, ()> {
            let rep = self.body_slots.len() as u32;
            self.body_slots.push(());
            Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
        }
        fn finish(&mut self, _p1: wasmtime::component::Resource<IncomingBody>) -> wasmtime::component::Resource<FutureTrailers> {
            let rep = self.body_slots.len() as u32;
            self.body_slots.push(());
            unsafe { wasmtime::component::Resource::new_own(rep) }
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<IncomingBody>) -> wasmtime::Result<()> {
            Ok(())
        }
    }

    impl HostFutureTrailers for Ctx {
        fn subscribe(&mut self, _p1: wasmtime::component::Resource<FutureTrailers>) -> wasmtime::component::Resource<Pollable> {
            unimplemented!("harness stub: wasi::http::types::HostFutureTrailers::subscribe")
        }
        fn get(&mut self, _p1: wasmtime::component::Resource<FutureTrailers>) -> Option<Result<Result<Option<wasmtime::component::Resource<Trailers>>, ErrorCode>, ()>> {
            unimplemented!("harness stub: wasi::http::types::HostFutureTrailers::get")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<FutureTrailers>) -> wasmtime::Result<()> {
            Ok(())
        }
    }

    impl HostOutgoingResponse for Ctx {
        fn new(&mut self, _p1: wasmtime::component::Resource<Headers>) -> wasmtime::component::Resource<OutgoingResponse> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingResponse::new")
        }
        fn status_code(&mut self, _p1: wasmtime::component::Resource<OutgoingResponse>) -> StatusCode {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingResponse::status_code")
        }
        fn set_status_code(&mut self, _p1: wasmtime::component::Resource<OutgoingResponse>, _p2: StatusCode) -> Result<(), ()> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingResponse::set_status_code")
        }
        fn headers(&mut self, _p1: wasmtime::component::Resource<OutgoingResponse>) -> wasmtime::component::Resource<Headers> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingResponse::headers")
        }
        fn body(&mut self, _p1: wasmtime::component::Resource<OutgoingResponse>) -> Result<wasmtime::component::Resource<OutgoingBody>, ()> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingResponse::body")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<OutgoingResponse>) -> wasmtime::Result<()> {
            unimplemented!("harness stub: wasi::http::types::HostOutgoingResponse::drop")
        }
    }

    impl HostOutgoingBody for Ctx {
        fn write(&mut self, _p1: wasmtime::component::Resource<OutgoingBody>) -> Result<wasmtime::component::Resource<OutputStream>, ()> {
            Ok(unsafe { wasmtime::component::Resource::new_own(0) })
        }
        fn finish(&mut self, _p1: wasmtime::component::Resource<OutgoingBody>, _p2: Option<wasmtime::component::Resource<Trailers>>) -> Result<(), ErrorCode> {
            Ok(())
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<OutgoingBody>) -> wasmtime::Result<()> {
            Ok(())
        }
    }

    impl HostFutureIncomingResponse for Ctx {
        fn subscribe(&mut self, _p1: wasmtime::component::Resource<FutureIncomingResponse>) -> wasmtime::component::Resource<Pollable> {
            unsafe { wasmtime::component::Resource::new_own(0) }
        }
        fn get(&mut self, _p1: wasmtime::component::Resource<FutureIncomingResponse>) -> Option<Result<Result<wasmtime::component::Resource<IncomingResponse>, ErrorCode>, ()>> {
            let rep = self.body_slots.len() as u32;
            self.body_slots.push(());
            Some(Ok(Ok(unsafe { wasmtime::component::Resource::new_own(rep) })))
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<FutureIncomingResponse>) -> wasmtime::Result<()> {
            Ok(())
        }
    }
}

pub mod stubs_wasi_io_error {
    use super::Ctx;
    use crate::wasi::io::error::*;

    impl HostError for Ctx {
        fn to_debug_string(&mut self, _p1: wasmtime::component::Resource<Error>) -> wasmtime::component::__internal::String {
            String::from("stub stream error")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<Error>) -> wasmtime::Result<()> {
            Ok(())
        }
    }
}

pub mod stubs_wasi_io_streams {
    use super::Ctx;
    use crate::wasi::io::streams::*;

    impl HostInputStream for Ctx {
        fn read(&mut self, _p1: wasmtime::component::Resource<InputStream>, _p2: u64) -> Result<wasmtime::component::__internal::Vec<u8>, StreamError> {
            let len = _p2 as usize;
            let start = self.response_cursor.min(self.response_body.len());
            let end = (start + len).min(self.response_body.len());
            let bytes = self.response_body[start..end].to_vec();
            self.response_cursor = end;
            Ok(bytes)
        }
        fn blocking_read(&mut self, _p1: wasmtime::component::Resource<InputStream>, _p2: u64) -> Result<wasmtime::component::__internal::Vec<u8>, StreamError> {
            let len = _p2 as usize;
            let start = self.response_cursor.min(self.response_body.len());
            let end = (start + len).min(self.response_body.len());
            let bytes = self.response_body[start..end].to_vec();
            self.response_cursor = end;
            Ok(bytes)
        }
        fn skip(&mut self, _p1: wasmtime::component::Resource<InputStream>, _p2: u64) -> Result<u64, StreamError> {
            unimplemented!("harness stub: wasi::io::streams::HostInputStream::skip")
        }
        fn blocking_skip(&mut self, _p1: wasmtime::component::Resource<InputStream>, _p2: u64) -> Result<u64, StreamError> {
            unimplemented!("harness stub: wasi::io::streams::HostInputStream::blocking_skip")
        }
        fn subscribe(&mut self, _p1: wasmtime::component::Resource<InputStream>) -> wasmtime::component::Resource<Pollable> {
            unsafe { wasmtime::component::Resource::new_own(0) }
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<InputStream>) -> wasmtime::Result<()> {
            Ok(())
        }
    }

    impl HostOutputStream for Ctx {
        fn check_write(&mut self, _p1: wasmtime::component::Resource<OutputStream>) -> Result<u64, StreamError> {
            Ok(4096)
        }
        fn write(&mut self, _p1: wasmtime::component::Resource<OutputStream>, _p2: wasmtime::component::__internal::Vec<u8>) -> Result<(), StreamError> {
            self.request_body.extend_from_slice(&_p2);
            Ok(())
        }
        fn blocking_write_and_flush(&mut self, _p1: wasmtime::component::Resource<OutputStream>, _p2: wasmtime::component::__internal::Vec<u8>) -> Result<(), StreamError> {
            self.request_body.extend_from_slice(&_p2);
            Ok(())
        }
        fn flush(&mut self, _p1: wasmtime::component::Resource<OutputStream>) -> Result<(), StreamError> {
            Ok(())
        }
        fn blocking_flush(&mut self, _p1: wasmtime::component::Resource<OutputStream>) -> Result<(), StreamError> {
            Ok(())
        }
        fn subscribe(&mut self, _p1: wasmtime::component::Resource<OutputStream>) -> wasmtime::component::Resource<Pollable> {
            unimplemented!("harness stub: wasi::io::streams::HostOutputStream::subscribe")
        }
        fn write_zeroes(&mut self, _p1: wasmtime::component::Resource<OutputStream>, _p2: u64) -> Result<(), StreamError> {
            unimplemented!("harness stub: wasi::io::streams::HostOutputStream::write_zeroes")
        }
        fn blocking_write_zeroes_and_flush(&mut self, _p1: wasmtime::component::Resource<OutputStream>, _p2: u64) -> Result<(), StreamError> {
            unimplemented!("harness stub: wasi::io::streams::HostOutputStream::blocking_write_zeroes_and_flush")
        }
        fn splice(&mut self, _p1: wasmtime::component::Resource<OutputStream>, _p2: wasmtime::component::Resource<InputStream>, _p3: u64) -> Result<u64, StreamError> {
            unimplemented!("harness stub: wasi::io::streams::HostOutputStream::splice")
        }
        fn blocking_splice(&mut self, _p1: wasmtime::component::Resource<OutputStream>, _p2: wasmtime::component::Resource<InputStream>, _p3: u64) -> Result<u64, StreamError> {
            unimplemented!("harness stub: wasi::io::streams::HostOutputStream::blocking_splice")
        }
        fn drop(&mut self, _p1: wasmtime::component::Resource<OutputStream>) -> wasmtime::Result<()> {
            Ok(())
        }
    }
}

// Interfaces whose only items are resources still expose an (empty) `Host`
// trait, which the generator skips because it has no methods.
impl crate::wasi::io::error::Host for Ctx {}
impl crate::wasi::io::streams::Host for Ctx {}

/// `wasi:http/types` also exposes one free function.
pub mod stubs_wasi_http_types_host {
    use super::Ctx;
    use crate::wasi::http::types::*;
    use crate::wasi::io::error::Error as IoError;

    impl Host for Ctx {
        fn http_error_code(&mut self, _p1: wasmtime::component::Resource<IoError>) -> Option<ErrorCode> {
            None
        }
    }
}
