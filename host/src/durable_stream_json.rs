//! The JSON mirror of `golem:agent/durable-streams`.
//!
//! Roc has neither resources nor variants with payloads, so — the way
//! `retry_json` mirrors the retry trees, `rdbms_json` the `db-value`
//! vocabularies and `permissions_json` the permission cards — every structured
//! value the durable-stream effects carry is JSON text in the WIT shape, with
//! the WIT names in camelCase:
//!
//! * a reader's options are `{"url":…,"mode":"json"|"bytes","timeoutMs":…}`
//!   and a writer's `{"url":…,"contentType":…,"producerId":…,
//!   "producerEpoch":…,"timeoutMs":…}`,
//! * a read request is `{"checkpoint":{"offset":…,"cursor":…|
//!   null},"transport":"catch-up"|"long-poll"|"sse","contentType":…|
//!   null}` (`cursor` and `contentType` may also be absent),
//! * a batch is `{"payload":…,"contentType":…,"next":{"offset":…,
//!   "cursor":…|null},"upToDate":…,"closed":…}`. The WIT's `payload` is a
//!   `list<u8>` and the platform carries bytes as lossy-UTF-8 text (the `http!`
//!   convention), so the text is what an app reads; `payloadBytes` is how many
//!   bytes the host actually received, which is how an app notices that the
//!   text lost something,
//! * an append request is `{"payload":{"tag":"json","value":["…", …]}|
//!   {"tag":"bytes","value":"…"},"sequence":…,"close":…}` — a JSON
//!   payload is individually encoded complete values (the host frames the outer
//!   array), and a byte payload is the text's UTF-8 bytes,
//! * an append receipt is `{"nextOffset":…|null,"epoch":…,"sequence":…,
//!   "closed":…}`,
//! * a `durable-stream-error` becomes the text `<kind>: <message>`, with
//!   `(retryAfterMs=…)`, `(producerEpoch=…)` and `(expectedSequence=…)`
//!   appended when the variant carries them — everything an app needs to decide
//!   how to recover.
//!
//! What the host enforces and this mirror keeps an app out of: a whole-attempt
//! deadline is 1 through 300000 ms, and a producer epoch or sequence stays at or
//! below 2^53-1. Both are checked here, before the call, so a typo is the
//! effect's `Err` rather than a host failure.

use serde::{Deserialize, Serialize};

use crate::golem::agent::durable_streams as wit;

/// The WIT's documented deadline range (`durable-stream-reader-options`).
pub const MIN_TIMEOUT_MS: u64 = 1;
pub const MAX_TIMEOUT_MS: u64 = 300_000;
/// The largest producer epoch and sequence the host accepts (the same ceiling
/// the Rust SDK puts on them).
pub const MAX_PRODUCER_NUMBER: u64 = (1 << 53) - 1;

/// `durable-stream-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeJson {
    Json,
    Bytes,
}

impl From<ModeJson> for wit::DurableStreamMode {
    fn from(mode: ModeJson) -> Self {
        match mode {
            ModeJson::Json => wit::DurableStreamMode::Json,
            ModeJson::Bytes => wit::DurableStreamMode::Bytes,
        }
    }
}

/// `durable-stream-transport`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransportJson {
    CatchUp,
    LongPoll,
    Sse,
}

impl From<TransportJson> for wit::DurableStreamTransport {
    fn from(transport: TransportJson) -> Self {
        match transport {
            TransportJson::CatchUp => wit::DurableStreamTransport::CatchUp,
            TransportJson::LongPoll => wit::DurableStreamTransport::LongPoll,
            TransportJson::Sse => wit::DurableStreamTransport::Sse,
        }
    }
}

/// `durable-stream-reader-options`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReaderOptionsJson {
    pub url: String,
    pub mode: ModeJson,
    pub timeout_ms: u64,
}

/// `durable-stream-writer-options`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriterOptionsJson {
    pub url: String,
    pub content_type: String,
    pub producer_id: String,
    pub producer_epoch: u64,
    pub timeout_ms: u64,
}

/// `durable-stream-checkpoint`: two server-generated opaque tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckpointJson {
    pub offset: String,
    #[serde(default)]
    pub cursor: Option<String>,
}

impl From<&CheckpointJson> for wit::DurableStreamCheckpoint {
    fn from(checkpoint: &CheckpointJson) -> Self {
        Self {
            offset: checkpoint.offset.clone(),
            cursor: checkpoint.cursor.clone(),
        }
    }
}

impl From<&wit::DurableStreamCheckpoint> for CheckpointJson {
    fn from(checkpoint: &wit::DurableStreamCheckpoint) -> Self {
        Self {
            offset: checkpoint.offset.clone(),
            cursor: checkpoint.cursor.clone(),
        }
    }
}

/// `durable-stream-read-request`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadRequestJson {
    pub checkpoint: CheckpointJson,
    pub transport: TransportJson,
    #[serde(default)]
    pub content_type: Option<String>,
}

impl From<&ReadRequestJson> for wit::DurableStreamReadRequest {
    fn from(request: &ReadRequestJson) -> Self {
        Self {
            checkpoint: (&request.checkpoint).into(),
            transport: request.transport.into(),
            content_type: request.content_type.clone(),
        }
    }
}

/// `durable-stream-batch`, as the effect hands it to an app: the WIT record
/// plus `payloadBytes`, the byte length of the payload the host received (the
/// text is lossy UTF-8, so this is how a difference shows).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchJson {
    pub payload: String,
    pub payload_bytes: u64,
    pub content_type: String,
    pub next: CheckpointJson,
    pub up_to_date: bool,
    pub closed: bool,
}

impl From<&wit::DurableStreamBatch> for BatchJson {
    fn from(batch: &wit::DurableStreamBatch) -> Self {
        Self {
            payload: String::from_utf8_lossy(&batch.payload).into_owned(),
            payload_bytes: batch.payload.len() as u64,
            content_type: batch.content_type.clone(),
            next: (&batch.next).into(),
            up_to_date: batch.up_to_date,
            closed: batch.closed,
        }
    }
}

/// `durable-stream-append-payload`: individually encoded JSON values, or the
/// bytes of a text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tag", content = "value", rename_all = "lowercase")]
pub enum AppendPayloadJson {
    Json(Vec<String>),
    Bytes(String),
}

impl From<&AppendPayloadJson> for wit::DurableStreamAppendPayload {
    fn from(payload: &AppendPayloadJson) -> Self {
        match payload {
            AppendPayloadJson::Json(values) => {
                wit::DurableStreamAppendPayload::Json(values.clone())
            }
            AppendPayloadJson::Bytes(text) => {
                wit::DurableStreamAppendPayload::Bytes(text.as_bytes().to_vec())
            }
        }
    }
}

/// `durable-stream-append-request`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppendRequestJson {
    pub payload: AppendPayloadJson,
    pub sequence: u64,
    pub close: bool,
}

/// `durable-stream-append-receipt`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptJson {
    pub next_offset: Option<String>,
    pub epoch: u64,
    pub sequence: u64,
    pub closed: bool,
}

impl From<&wit::DurableStreamAppendReceipt> for ReceiptJson {
    fn from(receipt: &wit::DurableStreamAppendReceipt) -> Self {
        Self {
            next_offset: receipt.next_offset.clone(),
            epoch: receipt.epoch,
            sequence: receipt.sequence,
            closed: receipt.closed,
        }
    }
}

/// `durable-stream-error`, as the text `<kind>: <message>` — the rendering the
/// `golem:rdbms`, webhook and permission errors already use — with the recovery
/// fields the variant may carry appended, so an app can act on a rate limit, a
/// fenced producer or an expected sequence without losing them.
pub fn error_text(error: &wit::DurableStreamError) -> String {
    let mut text = format!("{}: {}", error_kind_text(&error.kind), error.message);
    if let Some(retry_after_ms) = error.retry_after_ms {
        text.push_str(&format!(" (retryAfterMs={retry_after_ms})"));
    }
    if let Some(producer_epoch) = error.producer_epoch {
        text.push_str(&format!(" (producerEpoch={producer_epoch})"));
    }
    if let Some(expected_sequence) = error.expected_sequence {
        text.push_str(&format!(" (expectedSequence={expected_sequence})"));
    }
    text
}

/// The WIT case name of an error kind (`invalid-request`, `sequence-conflict`,
/// …), which is what the rendered error starts with.
pub fn error_kind_text(kind: &wit::DurableStreamErrorKind) -> &'static str {
    match kind {
        wit::DurableStreamErrorKind::InvalidRequest => "invalid-request",
        wit::DurableStreamErrorKind::PermissionDenied => "permission-denied",
        wit::DurableStreamErrorKind::NotFound => "not-found",
        wit::DurableStreamErrorKind::Gone => "gone",
        wit::DurableStreamErrorKind::Closed => "closed",
        wit::DurableStreamErrorKind::SequenceConflict => "sequence-conflict",
        wit::DurableStreamErrorKind::Fenced => "fenced",
        wit::DurableStreamErrorKind::ProducerDiverged => "producer-diverged",
        wit::DurableStreamErrorKind::ProtocolError => "protocol-error",
        wit::DurableStreamErrorKind::PayloadTooLarge => "payload-too-large",
        wit::DurableStreamErrorKind::Timeout => "timeout",
        wit::DurableStreamErrorKind::Transport => "transport",
        wit::DurableStreamErrorKind::RateLimited => "rate-limited",
        wit::DurableStreamErrorKind::Unavailable => "unavailable",
    }
}

/// The reader options of `durableStreamReader!`, as the WIT record the resource
/// constructor takes.
pub fn reader_options_from_json(text: &str) -> Result<wit::DurableStreamReaderOptions, String> {
    let options: ReaderOptionsJson = serde_json::from_str(text)
        .map_err(|err| format!("invalid durable stream reader options JSON: {err}"))?;
    check_timeout(options.timeout_ms, "reader")?;
    Ok(wit::DurableStreamReaderOptions {
        url: options.url,
        mode: options.mode.into(),
        timeout_ms: options.timeout_ms,
    })
}

/// The writer options of `durableStreamWriter!`. The producer epoch is checked
/// against the same ceiling the host applies, so an unusable identity is the
/// effect's `Err` instead of a failed append.
pub fn writer_options_from_json(text: &str) -> Result<wit::DurableStreamWriterOptions, String> {
    let options: WriterOptionsJson = serde_json::from_str(text)
        .map_err(|err| format!("invalid durable stream writer options JSON: {err}"))?;
    check_timeout(options.timeout_ms, "writer")?;
    if options.producer_epoch > MAX_PRODUCER_NUMBER {
        return Err(producer_number_error("producerEpoch", options.producer_epoch));
    }
    Ok(wit::DurableStreamWriterOptions {
        url: options.url,
        content_type: options.content_type,
        producer_id: options.producer_id,
        producer_epoch: options.producer_epoch,
        timeout_ms: options.timeout_ms,
    })
}

/// The read request of `durableStreamRead!`.
pub fn read_request_from_json(text: &str) -> Result<wit::DurableStreamReadRequest, String> {
    let request: ReadRequestJson = serde_json::from_str(text)
        .map_err(|err| format!("invalid durable stream read request JSON: {err}"))?;
    Ok((&request).into())
}

/// The append request of `durableStreamAppend!`. The sequence is checked the
/// way the writer's epoch is.
pub fn append_request_from_json(text: &str) -> Result<wit::DurableStreamAppendRequest, String> {
    let request: AppendRequestJson = serde_json::from_str(text)
        .map_err(|err| format!("invalid durable stream append request JSON: {err}"))?;
    if request.sequence > MAX_PRODUCER_NUMBER {
        return Err(producer_number_error("sequence", request.sequence));
    }
    Ok(wit::DurableStreamAppendRequest {
        payload: (&request.payload).into(),
        sequence: request.sequence,
        close: request.close,
    })
}

/// A batch as the effect's JSON.
pub fn batch_to_json(batch: &wit::DurableStreamBatch) -> String {
    to_json(&BatchJson::from(batch))
}

/// A receipt as the effect's JSON.
pub fn receipt_to_json(receipt: &wit::DurableStreamAppendReceipt) -> String {
    to_json(&ReceiptJson::from(receipt))
}

fn check_timeout(timeout_ms: u64, which: &str) -> Result<(), String> {
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(format!(
            "invalid durable stream {which} timeout: timeoutMs {timeout_ms} is outside {MIN_TIMEOUT_MS}..={MAX_TIMEOUT_MS}"
        ));
    }
    Ok(())
}

fn producer_number_error(field: &str, value: u64) -> String {
    format!(
        "invalid durable stream producer identity: {field} {value} exceeds {MAX_PRODUCER_NUMBER} (2^53-1)"
    )
}

/// The encoder every one of these shapes goes through, so the field order is
/// the encoder's (and stable) rather than hand-written per shape.
fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| String::from("null"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(offset: &str, cursor: Option<&str>) -> wit::DurableStreamCheckpoint {
        wit::DurableStreamCheckpoint {
            offset: offset.to_string(),
            cursor: cursor.map(|cursor| cursor.to_string()),
        }
    }

    fn error(kind: wit::DurableStreamErrorKind, message: &str) -> wit::DurableStreamError {
        wit::DurableStreamError {
            kind,
            message: message.to_string(),
            retry_after_ms: None,
            producer_epoch: None,
            expected_sequence: None,
        }
    }

    /// The reader options are the WIT record in camelCase, and the mode is the
    /// WIT enum's case name.
    #[test]
    fn test_reader_options_json() {
        let options = reader_options_from_json(
            r#"{"url":"http://127.0.0.1:4437/v1/stream/orders","mode":"json","timeoutMs":30000}"#,
        )
        .expect("the documented shape parses");
        assert_eq!(options.url, "http://127.0.0.1:4437/v1/stream/orders");
        assert_eq!(options.mode, wit::DurableStreamMode::Json);
        assert_eq!(options.timeout_ms, 30_000);

        let bytes = reader_options_from_json(
            r#"{"url":"https://example.test/s","mode":"bytes","timeoutMs":1}"#,
        )
        .expect("bytes mode parses");
        assert_eq!(bytes.mode, wit::DurableStreamMode::Bytes);
        assert_eq!(bytes.timeout_ms, MIN_TIMEOUT_MS);

        let upper = reader_options_from_json(
            r#"{"url":"https://example.test/s","mode":"json","timeoutMs":300000}"#,
        )
        .expect("the upper bound parses");
        assert_eq!(upper.timeout_ms, MAX_TIMEOUT_MS);
    }

    /// A mode the WIT does not have, an unknown field and a missing field are
    /// all reported as JSON the options text cannot be read as — never as a
    /// half-built reader.
    #[test]
    fn test_reader_options_json_rejections() {
        for (label, text) in [
            (
                "an unknown mode",
                r#"{"url":"https://e.test/s","mode":"events","timeoutMs":10}"#,
            ),
            (
                "an unknown field",
                r#"{"url":"https://e.test/s","mode":"json","timeoutMs":10,"live":true}"#,
            ),
            (
                "a missing timeout",
                r#"{"url":"https://e.test/s","mode":"json"}"#,
            ),
        ] {
            let error = reader_options_from_json(text).expect_err(label);
            assert!(
                error.starts_with("invalid durable stream reader options JSON: "),
                "{label} reported: {error}"
            );
        }
    }

    /// The deadline is the WIT's documented range; both ends and the middle
    /// parse, and one below or above is reported with the range.
    #[test]
    fn test_timeout_validation() {
        let text = |timeout_ms: u64| {
            format!(
                r#"{{"url":"https://e.test/s","mode":"json","timeoutMs":{timeout_ms}}}"#
            )
        };
        for timeout_ms in [MIN_TIMEOUT_MS, 42, MAX_TIMEOUT_MS] {
            assert!(
                reader_options_from_json(&text(timeout_ms)).is_ok(),
                "{timeout_ms} is inside the range"
            );
        }
        let low = reader_options_from_json(&text(0)).expect_err("0 is below the range");
        assert_eq!(
            low,
            "invalid durable stream reader timeout: timeoutMs 0 is outside 1..=300000"
        );
        let high = reader_options_from_json(&text(MAX_TIMEOUT_MS + 1))
            .expect_err("300001 is above the range");
        assert!(high.contains("timeoutMs 300001 is outside 1..=300000"), "{high}");
    }

    /// The writer options carry the producer identity the app owns: the id, the
    /// epoch and the content type, all camelCase.
    #[test]
    fn test_writer_options_json() {
        let options = writer_options_from_json(
            r#"{"url":"https://example.test/orders","contentType":"application/json","producerId":"p-1","producerEpoch":7,"timeoutMs":5000}"#,
        )
        .expect("the documented shape parses");
        assert_eq!(options.url, "https://example.test/orders");
        assert_eq!(options.content_type, "application/json");
        assert_eq!(options.producer_id, "p-1");
        assert_eq!(options.producer_epoch, 7);
        assert_eq!(options.timeout_ms, 5_000);

        let error = writer_options_from_json(
            r#"{"url":"https://e.test/s","contentType":"application/json","producerId":"p","producerEpoch":0}"#,
        )
        .expect_err("a missing timeout is reported");
        assert!(
            error.starts_with("invalid durable stream writer options JSON: "),
            "{error}"
        );
    }

    /// The epoch ceiling is the host's own (2^53-1): the boundary is accepted
    /// and one above it is reported before any call.
    #[test]
    fn test_producer_epoch_ceiling() {
        let text = |epoch: u64| {
            format!(
                r#"{{"url":"https://e.test/s","contentType":"application/json","producerId":"p","producerEpoch":{epoch},"timeoutMs":1000}}"#
            )
        };
        assert!(writer_options_from_json(&text(MAX_PRODUCER_NUMBER)).is_ok());
        let error = writer_options_from_json(&text(MAX_PRODUCER_NUMBER + 1))
            .expect_err("one above the ceiling is reported");
        assert_eq!(
            error,
            "invalid durable stream producer identity: producerEpoch 9007199254740992 exceeds 9007199254740991 (2^53-1)"
        );
    }

    /// A read request carries the checkpoint, the transport and the optional
    /// content type; `cursor` and `contentType` may be `null` or absent, and all
    /// three transports have their WIT names.
    #[test]
    fn test_read_request_json() {
        let request = read_request_from_json(
            r#"{"checkpoint":{"offset":"-1","cursor":null},"transport":"catch-up","contentType":null}"#,
        )
        .expect("the documented shape parses");
        assert_eq!(request.checkpoint.offset, "-1");
        assert_eq!(request.checkpoint.cursor, None);
        assert_eq!(request.transport, wit::DurableStreamTransport::CatchUp);
        assert_eq!(request.content_type, None);

        let long_poll = read_request_from_json(
            r#"{"checkpoint":{"offset":"0000000000000000_0000000000000026","cursor":"c-1"},"transport":"long-poll","contentType":"application/json"}"#,
        )
        .expect("a live read parses");
        assert_eq!(
            long_poll.transport,
            wit::DurableStreamTransport::LongPoll
        );
        assert_eq!(long_poll.checkpoint.cursor.as_deref(), Some("c-1"));
        assert_eq!(long_poll.content_type.as_deref(), Some("application/json"));

        let sse = read_request_from_json(
            r#"{"checkpoint":{"offset":"-1"},"transport":"sse","contentType":"application/json"}"#,
        )
        .expect("an SSE read parses");
        assert_eq!(sse.transport, wit::DurableStreamTransport::Sse);

        let error = read_request_from_json(
            r#"{"checkpoint":{"offset":"-1"},"transport":"poll"}"#,
        )
        .expect_err("a transport the WIT does not have is reported");
        assert!(
            error.starts_with("invalid durable stream read request JSON: "),
            "{error}"
        );
    }

    /// An append request's payload is the tagged union: JSON values stay
    /// individual, and byte payloads become the text's UTF-8 bytes.
    #[test]
    fn test_append_request_json() {
        let json = append_request_from_json(
            r#"{"payload":{"tag":"json","value":["{\"n\":1}","plain"]},"sequence":0,"close":false}"#,
        )
        .expect("a JSON append parses");
        match json.payload {
            wit::DurableStreamAppendPayload::Json(values) => {
                assert_eq!(values, vec!["{\"n\":1}".to_string(), "plain".to_string()]);
            }
            other => panic!("a JSON payload became {other:?}"),
        }
        assert_eq!(json.sequence, 0);
        assert!(!json.close);

        let bytes = append_request_from_json(
            r#"{"payload":{"tag":"bytes","value":"hi\u00e9"},"sequence":3,"close":true}"#,
        )
        .expect("a byte append parses");
        match bytes.payload {
            wit::DurableStreamAppendPayload::Bytes(value) => {
                assert_eq!(value, "hié".as_bytes().to_vec());
            }
            other => panic!("a byte payload became {other:?}"),
        }
        assert_eq!(bytes.sequence, 3);
        assert!(bytes.close);

        let error = append_request_from_json(
            r#"{"payload":{"tag":"text","value":"hi"},"sequence":0,"close":false}"#,
        )
        .expect_err("a tag the WIT does not have is reported");
        assert!(
            error.starts_with("invalid durable stream append request JSON: "),
            "{error}"
        );

        let error = append_request_from_json(
            r#"{"payload":{"tag":"bytes","value":"hi"},"sequence":9007199254740992,"close":false}"#,
        )
        .expect_err("a sequence above the ceiling is reported");
        assert!(
            error.contains("sequence 9007199254740992 exceeds 9007199254740991"),
            "{error}"
        );
    }

    /// A batch comes back in the WIT shape with camelCase names, the payload as
    /// lossy text and `payloadBytes` as what the host really received — the
    /// pair that lets an app notice the text lost something.
    #[test]
    fn test_batch_json() {
        let batch = wit::DurableStreamBatch {
            payload: br#"["hello","world"]"#.to_vec(),
            content_type: "application/json".to_string(),
            next: checkpoint("0000000000000000_0000000000000026", None),
            up_to_date: true,
            closed: false,
        };
        assert_eq!(
            batch_to_json(&batch),
            r#"{"payload":"[\"hello\",\"world\"]","payloadBytes":17,"contentType":"application/json","next":{"offset":"0000000000000000_0000000000000026","cursor":null},"upToDate":true,"closed":false}"#
        );

        let lossy = wit::DurableStreamBatch {
            payload: vec![0x68, 0xff, 0x69],
            content_type: "application/octet-stream".to_string(),
            next: checkpoint("5", Some("c-2")),
            up_to_date: false,
            closed: true,
        };
        let rendered = batch_to_json(&lossy);
        // The byte that is not UTF-8 became the replacement character; the
        // byte count is what says so.
        assert!(
            rendered.contains("\"payload\":\"h\u{fffd}i\""),
            "the payload text is lossy: {rendered}"
        );
        assert!(
            rendered.contains(r#""payloadBytes":3"#),
            "the byte length is reported: {rendered}"
        );
        assert!(
            rendered.contains(r#""cursor":"c-2""#),
            "the cursor travels: {rendered}"
        );
        assert!(rendered.contains(r#""closed":true"#), "{rendered}");
    }

    /// A receipt keeps the offset optional (a duplicate acknowledgement may
    /// omit it) and always carries the epoch, the sequence and the closed flag.
    #[test]
    fn test_receipt_json() {
        let receipt = wit::DurableStreamAppendReceipt {
            next_offset: Some("0000000000000000_0000000000000013".to_string()),
            epoch: 0,
            sequence: 0,
            closed: false,
        };
        assert_eq!(
            receipt_to_json(&receipt),
            r#"{"nextOffset":"0000000000000000_0000000000000013","epoch":0,"sequence":0,"closed":false}"#
        );

        let duplicate = wit::DurableStreamAppendReceipt {
            next_offset: None,
            epoch: 4,
            sequence: 9,
            closed: true,
        };
        assert_eq!(
            receipt_to_json(&duplicate),
            r#"{"nextOffset":null,"epoch":4,"sequence":9,"closed":true}"#
        );
    }

    /// Every error kind renders as its WIT case name, so an app can branch on
    /// the text's prefix the way it branches on the other rendered errors.
    #[test]
    fn test_error_kind_text() {
        let kinds = [
            (wit::DurableStreamErrorKind::InvalidRequest, "invalid-request"),
            (
                wit::DurableStreamErrorKind::PermissionDenied,
                "permission-denied",
            ),
            (wit::DurableStreamErrorKind::NotFound, "not-found"),
            (wit::DurableStreamErrorKind::Gone, "gone"),
            (wit::DurableStreamErrorKind::Closed, "closed"),
            (
                wit::DurableStreamErrorKind::SequenceConflict,
                "sequence-conflict",
            ),
            (wit::DurableStreamErrorKind::Fenced, "fenced"),
            (
                wit::DurableStreamErrorKind::ProducerDiverged,
                "producer-diverged",
            ),
            (wit::DurableStreamErrorKind::ProtocolError, "protocol-error"),
            (
                wit::DurableStreamErrorKind::PayloadTooLarge,
                "payload-too-large",
            ),
            (wit::DurableStreamErrorKind::Timeout, "timeout"),
            (wit::DurableStreamErrorKind::Transport, "transport"),
            (wit::DurableStreamErrorKind::RateLimited, "rate-limited"),
            (wit::DurableStreamErrorKind::Unavailable, "unavailable"),
        ];
        for (kind, name) in kinds {
            assert_eq!(error_kind_text(&kind), name);
        }
    }

    /// The rendered error keeps the message and appends only the recovery
    /// fields the variant actually carries — a rate limit's wait, a fence's
    /// epoch, a sequence conflict's expected sequence.
    #[test]
    fn test_error_text() {
        assert_eq!(
            error_text(&error(
                wit::DurableStreamErrorKind::NotFound,
                "Durable Streams HTTP status 404"
            )),
            "not-found: Durable Streams HTTP status 404"
        );

        let mut limited = error(wit::DurableStreamErrorKind::RateLimited, "HTTP status 429");
        limited.retry_after_ms = Some(1_500);
        assert_eq!(
            error_text(&limited),
            "rate-limited: HTTP status 429 (retryAfterMs=1500)"
        );

        let mut fenced = error(wit::DurableStreamErrorKind::Fenced, "Stale producer epoch");
        fenced.producer_epoch = Some(3);
        fenced.expected_sequence = Some(12);
        assert_eq!(
            error_text(&fenced),
            "fenced: Stale producer epoch (producerEpoch=3) (expectedSequence=12)"
        );
    }
}
