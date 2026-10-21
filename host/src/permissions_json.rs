//! The JSON mirror of `golem:permissions`, and of the principal.
//!
//! Roc has neither resources nor variants with payloads, so — the way
//! `retry_json` mirrors the retry trees and `rdbms_json` the three `db-value`
//! vocabularies — every structured value the permission effects carry is JSON
//! text in the WIT shape, with the WIT names in camelCase:
//!
//! * a `pattern-grant` is `{"class":…,"owner":…,"recipient":…,"verb":…,
//!   "resourceId":…}`,
//! * a `card-view` — and a derivation's requested bounds — is the four
//!   quadrants `{"lowerPositive":[…],"lowerNegative":[…],"upperPositive":[…],
//!   "upperNegative":[…]}`; a `derive` request adds `"lifespan":
//!   {"iso8601":"P30D"}` (`null` or absent for a card that never expires),
//!   and a `derive-scope` request adds `"parents":[<handle>, …]` instead (the
//!   WIT `derive-scope` takes no lifespan, so one is rejected as a typo),
//! * a `holder` is `{"tag":"account"|"app"|"agent","val":…}`: the two
//!   `uuid` payloads as canonical UUID text and the `agent` one as the
//!   `{"componentId":…,"agentId":…}` object the agent-id effects print,
//! * a `permission-error` becomes the text `<case-name>: <message>`, the way
//!   the `golem:rdbms` and webhook errors are rendered (`not-permitted: …`,
//!   `lower-bound-too-broad: …`).
//!
//! The principal the runtime hands `initialize` and `invoke` is mirrored the
//! same way, in the shape the Rust SDK's principal serde uses:
//! `{"tag":"anonymous"}`, `{"tag":"golem-user","val":{"accountId":…}}`,
//! `{"tag":"agent","val":{"componentId":…,"agentId":…}}` (the WIT's
//! nested `agent-id` record flattened, so the same object names an agent here,
//! in a `holder`, and in `resolveAgentId!`), and
//! `{"tag":"oidc","val":{"sub":…,"issuer":…,…}}` with every field the
//! runtime supplies.

use serde::{Deserialize, Serialize};

use crate::exports::golem::agent::guest::Principal;
use crate::golem::agent::common::OidcPrincipal;
use crate::golem::core::types as core;
use crate::golem::permissions::kernel_introspection as kernel;
use crate::golem::permissions::types as types;

/// A `pattern-grant`: the five fields naming one grant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatternGrantJson {
    pub class: String,
    pub owner: String,
    pub recipient: String,
    pub verb: String,
    pub resource_id: String,
}

impl From<&types::PatternGrant> for PatternGrantJson {
    fn from(grant: &types::PatternGrant) -> Self {
        Self {
            class: grant.class.clone(),
            owner: grant.owner.clone(),
            recipient: grant.recipient.clone(),
            verb: grant.verb.clone(),
            resource_id: grant.resource_id.clone(),
        }
    }
}

impl From<&PatternGrantJson> for types::PatternGrant {
    fn from(grant: &PatternGrantJson) -> Self {
        Self {
            class: grant.class.clone(),
            owner: grant.owner.clone(),
            recipient: grant.recipient.clone(),
            verb: grant.verb.clone(),
            resource_id: grant.resource_id.clone(),
        }
    }
}

/// A `duration`: the ISO-8601 string a lifespan is written as (`"P30D"`,
/// `"PT1H30M"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DurationJson {
    pub iso_8601: String,
}

/// The four quadrants of a card's lattice, as `inspect-card` answers them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardViewJson {
    pub lower_positive: Vec<PatternGrantJson>,
    pub lower_negative: Vec<PatternGrantJson>,
    pub upper_positive: Vec<PatternGrantJson>,
    pub upper_negative: Vec<PatternGrantJson>,
}

/// A persistent derivation's request: the grants to retain on the lower and
/// upper bound, the negatives to add, and an optional lifespan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundsJson {
    pub lower_positive: Vec<PatternGrantJson>,
    pub lower_negative: Vec<PatternGrantJson>,
    pub upper_positive: Vec<PatternGrantJson>,
    pub upper_negative: Vec<PatternGrantJson>,
    #[serde(default)]
    pub lifespan: Option<DurationJson>,
}

/// `derive-scope`'s request: the same four grant lists, plus the handles of
/// the persistent or scope cards the invocation-local card is derived from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopeRequestJson {
    pub parents: Vec<u32>,
    pub lower_positive: Vec<PatternGrantJson>,
    pub lower_negative: Vec<PatternGrantJson>,
    pub upper_positive: Vec<PatternGrantJson>,
    pub upper_negative: Vec<PatternGrantJson>,
}

/// The four grant lists of a request, in the WIT types the interfaces take.
pub struct GrantSets {
    pub lower_positive: Vec<types::PatternGrant>,
    pub lower_negative: Vec<types::PatternGrant>,
    pub upper_positive: Vec<types::PatternGrant>,
    pub upper_negative: Vec<types::PatternGrant>,
}

fn wit_grants(grants: &[PatternGrantJson]) -> Vec<types::PatternGrant> {
    grants.iter().map(types::PatternGrant::from).collect()
}

impl CardViewJson {
    /// The four lists as `inspect-card` answered them.
    pub fn to_json(view: &types::CardView) -> String {
        to_json(&Self {
            lower_positive: view.lower_positive.iter().map(Into::into).collect(),
            lower_negative: view.lower_negative.iter().map(Into::into).collect(),
            upper_positive: view.upper_positive.iter().map(Into::into).collect(),
            upper_negative: view.upper_negative.iter().map(Into::into).collect(),
        })
    }
}

impl BoundsJson {
    /// The request's grant lists and lifespan in the WIT types.
    pub fn grants(&self) -> GrantSets {
        GrantSets {
            lower_positive: wit_grants(&self.lower_positive),
            lower_negative: wit_grants(&self.lower_negative),
            upper_positive: wit_grants(&self.upper_positive),
            upper_negative: wit_grants(&self.upper_negative),
        }
    }

    /// The request's lifespan, when it named one.
    pub fn lifespan(&self) -> Option<types::Duration> {
        self.lifespan
            .as_ref()
            .map(|duration| types::Duration {
                iso_8601: duration.iso_8601.clone(),
            })
    }
}

impl ScopeRequestJson {
    /// The request's grant lists in the WIT types (it has no lifespan: the
    /// invocation-local card it asks for lives exactly as long as the
    /// invocation that derives it).
    pub fn grants(&self) -> GrantSets {
        GrantSets {
            lower_positive: wit_grants(&self.lower_positive),
            lower_negative: wit_grants(&self.lower_negative),
            upper_positive: wit_grants(&self.upper_positive),
            upper_negative: wit_grants(&self.upper_negative),
        }
    }
}

/// A `holder`: the wallet a card is installed onto. The two UUID payloads are
/// canonical UUID text, and the `agent` one is the same object an agent-id
/// record renders as.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tag", content = "val", rename_all = "camelCase")]
pub enum HolderJson {
    Account(String),
    App(String),
    Agent(AgentIdJson),
}

/// An `agent-id` record: the component the agent belongs to plus the agent id
/// string, the shape `resolveAgentId!` answers with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentIdJson {
    pub component_id: String,
    pub agent_id: String,
}

/// The agent-id record as the object the resolver, an `agent` holder and the
/// principal all print. One definition, because they all mean the same thing.
fn agent_id_object(id: &core::AgentId) -> AgentIdJson {
    AgentIdJson {
        component_id: crate::uuid_string(&id.component_id.uuid),
        agent_id: id.agent_id.clone(),
    }
}

/// The agent-id record as JSON text.
pub fn agent_id_json(id: &core::AgentId) -> String {
    to_json(&agent_id_object(id))
}

impl HolderJson {
    /// The holder in the WIT type, or why the text cannot be read as one.
    pub fn into_wit(self) -> Result<types::Holder, String> {
        match self {
            HolderJson::Account(text) => Ok(types::Holder::Account(uuid_from_text(&text)?)),
            HolderJson::App(text) => Ok(types::Holder::App(uuid_from_text(&text)?)),
            HolderJson::Agent(id) => Ok(types::Holder::Agent(core::AgentId {
                component_id: core::ComponentId {
                    uuid: uuid_from_text(&id.component_id)?,
                },
                agent_id: id.agent_id,
            })),
        }
    }
}

/// A `uuid` from its canonical text.
fn uuid_from_text(text: &str) -> Result<core::Uuid, String> {
    let (high_bits, low_bits) = crate::rdbms_json::uuid_bits(text)?;
    Ok(core::Uuid { high_bits, low_bits })
}

/// A `card-id` as JSON: the canonical UUID text, as a JSON string. The effects
/// that answer with an id or a list of ids hand back JSON like every other
/// permission effect, so an app reads them with the same reader.
pub fn card_id_json(id: &core::CardId) -> String {
    to_json(&crate::uuid_string(&id.uuid))
}

/// A list of `card-id`s as a JSON array of canonical UUID strings.
pub fn card_ids_json(ids: &[core::CardId]) -> String {
    to_json(
        &ids.iter()
            .map(|id| crate::uuid_string(&id.uuid))
            .collect::<Vec<String>>(),
    )
}

/// A `timestamp` as JSON, or the literal `null` for a card that never expires
/// (`expires-at` answers an `option`).
pub fn timestamp_json(timestamp: Option<&types::Timestamp>) -> String {
    match timestamp {
        Some(timestamp) => to_json(&serde_json::json!({
            "seconds": timestamp.seconds,
            "nanoseconds": timestamp.nanoseconds,
        })),
        None => String::from("null"),
    }
}

/// One card in the caller's wallet: the kind it is listed as, and the handle
/// the card's own effects take (`concrete`, `template` or `scope`).
pub fn wallet_card_json(tag: &str, handle: u32) -> String {
    to_json(&serde_json::json!({ "tag": tag, "handle": handle }))
}

/// A `wallet-version-token` as JSON: the wallet's id hash as an array of byte
/// numbers and the generation it is at.
pub fn wallet_version_json(token: &types::WalletVersionToken) -> String {
    to_json(&serde_json::json!({
        "walletIdHash": token.wallet_id_hash,
        "generation": token.generation,
    }))
}

/// The kernel's resource classes as a JSON array of
/// `{"className":…,"verbs":[…]}`, in the order the kernel listed them.
pub fn kernel_modules_json(modules: &[kernel::KernelModule]) -> String {
    let modules: Vec<serde_json::Value> = modules
        .iter()
        .map(|module| {
            serde_json::json!({
                "className": module.class_name,
                "verbs": module.verbs,
            })
        })
        .collect();
    to_json(&modules)
}

/// The `permission-error` variant for an effect's `Err` channel: its WIT case
/// name in kebab-case, then the host's message, the way `rdbms_json` renders
/// the database errors.
pub fn permission_error_text(error: &types::PermissionError) -> String {
    use types::PermissionError;
    match error {
        PermissionError::NotPermitted(message) => format!("not-permitted: {message}"),
        PermissionError::LowerBoundTooBroad(message) => {
            format!("lower-bound-too-broad: {message}")
        }
        PermissionError::UpperBoundTooBroad(message) => {
            format!("upper-bound-too-broad: {message}")
        }
        PermissionError::WalletMultiSourceRequired(message) => {
            format!("wallet-multi-source-required: {message}")
        }
        PermissionError::CardRevoked(message) => format!("card-revoked: {message}"),
        PermissionError::CardExpired(message) => format!("card-expired: {message}"),
        PermissionError::InstallRecipientMismatch(message) => {
            format!("install-recipient-mismatch: {message}")
        }
        PermissionError::UnknownResourceClass(message) => {
            format!("unknown-resource-class: {message}")
        }
        PermissionError::InvalidPattern(message) => format!("invalid-pattern: {message}"),
        PermissionError::InvalidVerb(message) => format!("invalid-verb: {message}"),
        PermissionError::InvalidOwner(message) => format!("invalid-owner: {message}"),
        PermissionError::InvalidRecipient(message) => {
            format!("invalid-recipient: {message}")
        }
    }
}

/// The `principal` the runtime handed this invocation, as JSON. Every field
/// the WIT record carries is kept; the `agent` case flattens the nested
/// `agent-id` record the way the Rust SDK's principal serde does.
pub fn principal_to_json(principal: &Principal) -> String {
    match principal {
        Principal::Anonymous => String::from(r#"{"tag":"anonymous"}"#),
        Principal::GolemUser(user) => to_json(&serde_json::json!({
            "tag": "golem-user",
            "val": { "accountId": crate::uuid_string(&user.account_id.uuid) },
        })),
        Principal::Agent(agent) => to_json(&serde_json::json!({
            "tag": "agent",
            "val": agent_id_object(&agent.agent_id),
        })),
        Principal::Oidc(oidc) => to_json(&serde_json::json!({
            "tag": "oidc",
            "val": oidc_fields(oidc),
        })),
    }
}

/// The `oidc-principal` record's fields, with the WIT names in camelCase and
/// an absent option as `null`.
fn oidc_fields(oidc: &OidcPrincipal) -> serde_json::Value {
    serde_json::json!({
        "sub": oidc.sub,
        "issuer": oidc.issuer,
        "email": oidc.email,
        "name": oidc.name,
        "emailVerified": oidc.email_verified,
        "givenName": oidc.given_name,
        "familyName": oidc.family_name,
        "picture": oidc.picture,
        "preferredUsername": oidc.preferred_username,
        "claims": oidc.claims,
    })
}

/// A `derive` request from the effect's text.
pub fn bounds_from_json(text: &str) -> Result<BoundsJson, String> {
    serde_json::from_str(text).map_err(|err| format!("invalid permission bounds JSON: {err}"))
}

/// A `derive-scope` request from the effect's text.
pub fn scope_request_from_json(text: &str) -> Result<ScopeRequestJson, String> {
    serde_json::from_str(text)
        .map_err(|err| format!("invalid permission scope JSON: {err}"))
}

/// A `holder` from the effect's text.
pub fn holder_from_json(text: &str) -> Result<types::Holder, String> {
    let holder: HolderJson = serde_json::from_str(text)
        .map_err(|err| format!("invalid permission holder JSON: {err}"))?;
    holder.into_wit()
}

/// One `pattern-grant` from the effect's text.
pub fn grant_from_json(text: &str) -> Result<types::PatternGrant, String> {
    let grant: PatternGrantJson = serde_json::from_str(text)
        .map_err(|err| format!("invalid permission grant JSON: {err}"))?;
    Ok(types::PatternGrant::from(&grant))
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("the JSON mirror always serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golem::agent::common::{AgentPrincipal, GolemUserPrincipal};

    fn grant(class: &str, verb: &str, resource_id: &str) -> types::PatternGrant {
        types::PatternGrant {
            class: class.to_string(),
            owner: String::from("?agent"),
            recipient: String::from("?agent"),
            verb: verb.to_string(),
            resource_id: resource_id.to_string(),
        }
    }

    fn agent_principal() -> core::AgentId {
        core::AgentId {
            component_id: core::ComponentId {
                uuid: core::Uuid {
                    high_bits: 0x0102_0304_0506_0708,
                    low_bits: 0x090a_0b0c_0d0e_0f10,
                },
            },
            agent_id: String::from("counter-agent(\"c-1\")"),
        }
    }

    /// A grant survives the trip in both directions with its WIT field names
    /// in camelCase (`resource-id` is the one that would be missed).
    #[test]
    fn test_pattern_grant_json_round_trip() {
        let wit = grant("filesystem", "read", "/workspace/**");
        let json = to_json(&PatternGrantJson::from(&wit));
        assert_eq!(
            json,
            r#"{"class":"filesystem","owner":"?agent","recipient":"?agent","verb":"read","resourceId":"/workspace/**"}"#
        );
        let parsed = grant_from_json(&json).expect("parses back");
        assert_eq!(parsed.class, wit.class);
        assert_eq!(parsed.owner, wit.owner);
        assert_eq!(parsed.recipient, wit.recipient);
        assert_eq!(parsed.verb, wit.verb);
        assert_eq!(parsed.resource_id, wit.resource_id);

        // A grant with a misspelled or missing field is a typo, not a default.
        let err = grant_from_json(r#"{"class":"filesystem","owner":"?agent","recipient":"?agent","verb":"read"}"#)
            .expect_err("no resource id");
        assert!(err.contains("resourceId"), "unexpected error: {err}");
        assert!(grant_from_json(r#"{"class":"f","owner":"o","recipient":"r","verb":"v","resource-id":"x"}"#).is_err());
    }

    /// A `card-view` renders all four quadrants, and a bounds object parses
    /// with an absent, a null or a written lifespan.
    #[test]
    fn test_card_view_and_bounds_json() {
        let view = types::CardView {
            lower_positive: vec![grant("filesystem", "read", "/workspace/**")],
            lower_negative: vec![],
            upper_positive: vec![grant("secret", "reveal", "*")],
            upper_negative: vec![grant("secret", "reveal", "billing.*")],
        };
        assert_eq!(
            CardViewJson::to_json(&view),
            concat!(
                r#"{"lowerPositive":[{"class":"filesystem","owner":"?agent","recipient":"?agent","verb":"read","resourceId":"/workspace/**"}],"#,
                r#""lowerNegative":[],"#,
                r#""upperPositive":[{"class":"secret","owner":"?agent","recipient":"?agent","verb":"reveal","resourceId":"*"}],"#,
                r#""upperNegative":[{"class":"secret","owner":"?agent","recipient":"?agent","verb":"reveal","resourceId":"billing.*"}]}"#,
            )
        );

        let bounds = bounds_from_json(
            r#"{"lowerPositive":[{"class":"filesystem","owner":"?agent","recipient":"?agent","verb":"read","resourceId":"/data/**"}],"lowerNegative":[],"upperPositive":[],"upperNegative":[],"lifespan":{"iso8601":"P30D"}}"#,
        )
        .expect("written lifespan");
        assert_eq!(bounds.grants().lower_positive.len(), 1);
        assert_eq!(
            bounds.lifespan().expect("a lifespan").iso_8601,
            "P30D"
        );

        // A lifespan that is absent, and one written as `null`, are the same
        // thing: a card that never expires.
        for text in [
            r#"{"lowerPositive":[],"lowerNegative":[],"upperPositive":[],"upperNegative":[]}"#,
            r#"{"lowerPositive":[],"lowerNegative":[],"upperPositive":[],"upperNegative":[],"lifespan":null}"#,
        ] {
            let bounds = bounds_from_json(text).expect("no lifespan");
            assert!(bounds.lifespan().is_none());
            assert!(bounds.grants().upper_negative.is_empty());
        }

        // A typo is reported, not ignored, and so is a quadrant that is missing.
        let err = bounds_from_json(
            r#"{"lowerPositive":[],"lowerNegative":[],"upperPositive":[],"upperNegative":[],"lifeSpan":null}"#,
        )
        .expect_err("misspelled lifespan");
        assert!(err.contains("lifeSpan"), "unexpected error: {err}");
        assert!(bounds_from_json(r#"{"lowerPositive":[]}"#).is_err());
        assert!(bounds_from_json("not json").is_err());
    }

    /// A `derive-scope` request carries the parent handles and no lifespan
    /// (the WIT `derive-scope` takes none, so a `lifespan` there is a typo).
    #[test]
    fn test_scope_request_json() {
        let request = scope_request_from_json(
            r#"{"parents":[1,2],"lowerPositive":[],"lowerNegative":[],"upperPositive":[],"upperNegative":[]}"#,
        )
        .expect("parents and bounds");
        assert_eq!(request.parents, vec![1, 2]);
        assert!(request.grants().lower_positive.is_empty());

        let err = scope_request_from_json(
            r#"{"parents":[1],"lowerPositive":[],"lowerNegative":[],"upperPositive":[],"upperNegative":[],"lifespan":{"iso8601":"PT1H"}}"#,
        )
        .expect_err("a lifespan the interface does not take");
        assert!(err.contains("lifespan"), "unexpected error: {err}");
    }

    /// A holder reads as the three wallets it can name: two UUIDs as canonical
    /// text and an agent as the agent-id object.
    #[test]
    fn test_holder_json() {
        let account = holder_from_json(
            r#"{"tag":"account","val":"01020304-0506-0708-090a-0b0c0d0e0f10"}"#,
        )
        .expect("an account holder");
        assert!(matches!(account, types::Holder::Account(uuid)
            if uuid.high_bits == 0x0102_0304_0506_0708 && uuid.low_bits == 0x090a_0b0c_0d0e_0f10));

        assert!(matches!(
            holder_from_json(r#"{"tag":"app","val":"01020304-0506-0708-090a-0b0c0d0e0f10"}"#)
                .expect("an app holder"),
            types::Holder::App(_)
        ));

        let agent = holder_from_json(&format!(
            r#"{{"tag":"agent","val":{}}}"#,
            agent_id_json(&agent_principal())
        ))
        .expect("an agent holder");
        match agent {
            types::Holder::Agent(id) => {
                assert_eq!(id.agent_id, "counter-agent(\"c-1\")");
                assert_eq!(id.component_id.uuid.high_bits, 0x0102_0304_0506_0708);
            }
            other => panic!("expected an agent holder, got {other:?}"),
        }

        // An unknown kind, a malformed UUID and a missing payload are errors.
        assert!(holder_from_json(r#"{"tag":"team","val":"x"}"#).is_err());
        assert!(holder_from_json(r#"{"tag":"account","val":"not-a-uuid"}"#).is_err());
        assert!(holder_from_json(r#"{"tag":"account"}"#).is_err());
    }

    /// Ids, timestamps, the wallet version and the kernel modules render as
    /// the JSON the effects promise.
    #[test]
    fn test_permission_json_rendering() {
        let card_id = core::CardId {
            uuid: core::Uuid {
                high_bits: 0x0102_0304_0506_0708,
                low_bits: 0x090a_0b0c_0d0e_0f10,
            },
        };
        assert_eq!(card_id_json(&card_id), r#""01020304-0506-0708-090a-0b0c0d0e0f10""#);
        assert_eq!(
            card_ids_json(&[card_id]),
            r#"["01020304-0506-0708-090a-0b0c0d0e0f10"]"#
        );
        assert_eq!(card_ids_json(&[]), "[]");

        let timestamp = types::Timestamp {
            seconds: 1_700_000_000,
            nanoseconds: 123,
        };
        assert_eq!(
            timestamp_json(Some(&timestamp)),
            r#"{"nanoseconds":123,"seconds":1700000000}"#
        );
        assert_eq!(timestamp_json(None), "null");

        assert_eq!(
            wallet_version_json(&types::WalletVersionToken {
                wallet_id_hash: vec![1, 2, 3],
                generation: 7,
            }),
            r#"{"generation":7,"walletIdHash":[1,2,3]}"#
        );

        assert_eq!(
            wallet_card_json("template", 3),
            r#"{"handle":3,"tag":"template"}"#
        );

        assert_eq!(
            kernel_modules_json(&[
                kernel::KernelModule {
                    class_name: String::from("filesystem"),
                    verbs: vec![String::from("read"), String::from("delete")],
                },
                kernel::KernelModule {
                    class_name: String::from("network"),
                    verbs: vec![],
                },
            ]),
            r#"[{"className":"filesystem","verbs":["read","delete"]},{"className":"network","verbs":[]}]"#
        );
        assert_eq!(kernel_modules_json(&[]), "[]");
    }

    /// Every `permission-error` case renders as its WIT case name and the
    /// host's message.
    #[test]
    fn test_permission_error_text() {
        assert_eq!(
            permission_error_text(&types::PermissionError::NotPermitted(String::from(
                "the caller holds no such card"
            ))),
            "not-permitted: the caller holds no such card"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::LowerBoundTooBroad(String::from(
                "read on /etc is not covered by the parent"
            ))),
            "lower-bound-too-broad: read on /etc is not covered by the parent"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::InstallRecipientMismatch(
                String::from("the recipient does not cover the target")
            )),
            "install-recipient-mismatch: the recipient does not cover the target"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::CardRevoked(String::from("gone"))),
            "card-revoked: gone"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::UnknownResourceClass(String::from(
                "nope"
            ))),
            "unknown-resource-class: nope"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::WalletMultiSourceRequired(
                String::from("one card only")
            )),
            "wallet-multi-source-required: one card only"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::UpperBoundTooBroad(String::from("wide"))),
            "upper-bound-too-broad: wide"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::CardExpired(String::from("old"))),
            "card-expired: old"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::InvalidPattern(String::from("slot"))),
            "invalid-pattern: slot"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::InvalidVerb(String::from("fly"))),
            "invalid-verb: fly"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::InvalidOwner(String::from("who"))),
            "invalid-owner: who"
        );
        assert_eq!(
            permission_error_text(&types::PermissionError::InvalidRecipient(String::from("them"))),
            "invalid-recipient: them"
        );
    }

    /// The principal renders in the four shapes the runtime hands over, with
    /// every field the OIDC record carries kept.
    #[test]
    fn test_principal_json() {
        assert_eq!(
            principal_to_json(&Principal::Anonymous),
            r#"{"tag":"anonymous"}"#
        );

        assert_eq!(
            principal_to_json(&Principal::GolemUser(GolemUserPrincipal {
                account_id: core::AccountId {
                    uuid: core::Uuid {
                        high_bits: 0x0102_0304_0506_0708,
                        low_bits: 0x090a_0b0c_0d0e_0f10,
                    },
                },
            })),
            r#"{"tag":"golem-user","val":{"accountId":"01020304-0506-0708-090a-0b0c0d0e0f10"}}"#
        );

        assert_eq!(
            principal_to_json(&Principal::Agent(AgentPrincipal {
                agent_id: agent_principal(),
            })),
            r#"{"tag":"agent","val":{"agentId":"counter-agent(\"c-1\")","componentId":"01020304-0506-0708-090a-0b0c0d0e0f10"}}"#
        );

        let oidc = principal_to_json(&Principal::Oidc(OidcPrincipal {
            sub: String::from("sub-1"),
            issuer: String::from("https://issuer.test"),
            email: Some(String::from("a@b.test")),
            name: Some(String::from("A B")),
            email_verified: Some(true),
            given_name: Some(String::from("A")),
            family_name: Some(String::from("B")),
            picture: None,
            preferred_username: Some(String::from("ab")),
            claims: String::from(r#"{"role":"admin"}"#),
        }));
        let parsed: serde_json::Value = serde_json::from_str(&oidc).expect("JSON");
        assert_eq!(parsed["tag"], "oidc");
        assert_eq!(parsed["val"]["sub"], "sub-1");
        assert_eq!(parsed["val"]["emailVerified"], true);
        assert_eq!(parsed["val"]["preferredUsername"], "ab");
        assert_eq!(parsed["val"]["claims"], r#"{"role":"admin"}"#);
        assert!(parsed["val"]["picture"].is_null());
    }
}
