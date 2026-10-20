//! The JSON mirror of `golem:api/retry`.
//!
//! The WIT interface models retry policies and predicates as flattened trees: a
//! `nodes` list whose root is `nodes[0]`, with children named by index. Roc has
//! neither resources nor recursive variants to hand such a tree to an app, so —
//! the way `http!` handes over a structured response — the platform renders the
//! tree as JSON text and parses the same shape back.
//!
//! The text is the WIT shape, not a translation of it:
//!
//! * a node is `{"tag": payload}`, or a bare string for the variants that carry
//!   no payload (`"immediate"`, `"predTrue"`); the WIT variant and field names
//!   appear in camelCase,
//! * a `duration` is its nanosecond count, as a JSON number,
//! * a `predicate-value` is a plain JSON string, number or boolean,
//! * node indices stay numbers, and the root stays at index 0.
//!
//! Everything the read effects print can be handed back to `setRetryPolicy!` and
//! `nextRetryDelay!` unchanged.

use serde::{Deserialize, Serialize};

use crate::golem::api::retry as wit;

/// A node index into a flattened tree (`nodes[0]` is the root).
type NodeIndex = i32;

/// A WASI p3 duration: nanoseconds.
type Nanos = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedPolicyJson {
    pub name: String,
    pub priority: u32,
    pub predicate: PredicateJson,
    pub policy: PolicyJson,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredicateJson {
    pub nodes: Vec<PredicateNodeJson>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyJson {
    pub nodes: Vec<PolicyNodeJson>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComparisonJson {
    pub property_name: String,
    /// A `predicate-value`: a JSON string, integer or boolean.
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetCheckJson {
    pub property_name: String,
    pub values: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatternJson {
    pub property_name: String,
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PredicateNodeJson {
    PropEq(ComparisonJson),
    PropNeq(ComparisonJson),
    PropGt(ComparisonJson),
    PropGte(ComparisonJson),
    PropLt(ComparisonJson),
    PropLte(ComparisonJson),
    PropExists(String),
    PropIn(SetCheckJson),
    PropMatches(PatternJson),
    PropStartsWith(PatternJson),
    PropContains(PatternJson),
    PredAnd((NodeIndex, NodeIndex)),
    PredOr((NodeIndex, NodeIndex)),
    PredNot(NodeIndex),
    PredTrue,
    PredFalse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExponentialConfigJson {
    pub base_delay: Nanos,
    pub factor: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FibonacciConfigJson {
    pub first: Nanos,
    pub second: Nanos,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CountBoxConfigJson {
    pub max_retries: u32,
    pub inner: NodeIndex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeBoxConfigJson {
    pub limit: Nanos,
    pub inner: NodeIndex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClampConfigJson {
    pub min_delay: Nanos,
    pub max_delay: Nanos,
    pub inner: NodeIndex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddDelayConfigJson {
    pub delay: Nanos,
    pub inner: NodeIndex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JitterConfigJson {
    pub factor: f64,
    pub inner: NodeIndex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilteredConfigJson {
    /// `filtered-on` embeds a whole predicate tree, not an index.
    pub predicate: PredicateJson,
    pub inner: NodeIndex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicyNodeJson {
    Periodic(Nanos),
    Exponential(ExponentialConfigJson),
    Fibonacci(FibonacciConfigJson),
    Immediate,
    Never,
    CountBox(CountBoxConfigJson),
    TimeBox(TimeBoxConfigJson),
    ClampDelay(ClampConfigJson),
    AddDelay(AddDelayConfigJson),
    Jitter(JitterConfigJson),
    FilteredOn(FilteredConfigJson),
    AndThen((NodeIndex, NodeIndex)),
    PolicyUnion((NodeIndex, NodeIndex)),
    PolicyIntersect((NodeIndex, NodeIndex)),
}

// ── Rendering: WIT → JSON ─────────────────────────────────────────────

impl From<&wit::PredicateValue> for serde_json::Value {
    fn from(value: &wit::PredicateValue) -> Self {
        match value {
            wit::PredicateValue::Text(text) => serde_json::Value::String(text.clone()),
            wit::PredicateValue::Integer(number) => serde_json::Value::from(*number),
            wit::PredicateValue::Boolean(flag) => serde_json::Value::Bool(*flag),
        }
    }
}

impl From<&wit::PropertyComparison> for ComparisonJson {
    fn from(comparison: &wit::PropertyComparison) -> Self {
        ComparisonJson {
            property_name: comparison.property_name.clone(),
            value: serde_json::Value::from(&comparison.value),
        }
    }
}

impl From<&wit::PropertySetCheck> for SetCheckJson {
    fn from(check: &wit::PropertySetCheck) -> Self {
        SetCheckJson {
            property_name: check.property_name.clone(),
            values: check.values.iter().map(serde_json::Value::from).collect(),
        }
    }
}

impl From<&wit::PropertyPattern> for PatternJson {
    fn from(pattern: &wit::PropertyPattern) -> Self {
        PatternJson {
            property_name: pattern.property_name.clone(),
            pattern: pattern.pattern.clone(),
        }
    }
}

impl From<&wit::PredicateNode> for PredicateNodeJson {
    fn from(node: &wit::PredicateNode) -> Self {
        match node {
            wit::PredicateNode::PropEq(comparison) => PredicateNodeJson::PropEq(comparison.into()),
            wit::PredicateNode::PropNeq(comparison) => {
                PredicateNodeJson::PropNeq(comparison.into())
            }
            wit::PredicateNode::PropGt(comparison) => PredicateNodeJson::PropGt(comparison.into()),
            wit::PredicateNode::PropGte(comparison) => {
                PredicateNodeJson::PropGte(comparison.into())
            }
            wit::PredicateNode::PropLt(comparison) => PredicateNodeJson::PropLt(comparison.into()),
            wit::PredicateNode::PropLte(comparison) => {
                PredicateNodeJson::PropLte(comparison.into())
            }
            wit::PredicateNode::PropExists(property_name) => {
                PredicateNodeJson::PropExists(property_name.clone())
            }
            wit::PredicateNode::PropIn(check) => PredicateNodeJson::PropIn(check.into()),
            wit::PredicateNode::PropMatches(pattern) => {
                PredicateNodeJson::PropMatches(pattern.into())
            }
            wit::PredicateNode::PropStartsWith(pattern) => {
                PredicateNodeJson::PropStartsWith(pattern.into())
            }
            wit::PredicateNode::PropContains(pattern) => {
                PredicateNodeJson::PropContains(pattern.into())
            }
            wit::PredicateNode::PredAnd((left, right)) => {
                PredicateNodeJson::PredAnd((*left, *right))
            }
            wit::PredicateNode::PredOr((left, right)) => PredicateNodeJson::PredOr((*left, *right)),
            wit::PredicateNode::PredNot(index) => PredicateNodeJson::PredNot(*index),
            wit::PredicateNode::PredTrue => PredicateNodeJson::PredTrue,
            wit::PredicateNode::PredFalse => PredicateNodeJson::PredFalse,
        }
    }
}

impl From<&wit::RetryPredicate> for PredicateJson {
    fn from(predicate: &wit::RetryPredicate) -> Self {
        PredicateJson {
            nodes: predicate
                .nodes
                .iter()
                .map(PredicateNodeJson::from)
                .collect(),
        }
    }
}

impl From<&wit::PolicyNode> for PolicyNodeJson {
    fn from(node: &wit::PolicyNode) -> Self {
        match node {
            wit::PolicyNode::Periodic(delay) => PolicyNodeJson::Periodic(*delay),
            wit::PolicyNode::Exponential(config) => {
                PolicyNodeJson::Exponential(ExponentialConfigJson {
                    base_delay: config.base_delay,
                    factor: config.factor,
                })
            }
            wit::PolicyNode::Fibonacci(config) => PolicyNodeJson::Fibonacci(FibonacciConfigJson {
                first: config.first,
                second: config.second,
            }),
            wit::PolicyNode::Immediate => PolicyNodeJson::Immediate,
            wit::PolicyNode::Never => PolicyNodeJson::Never,
            wit::PolicyNode::CountBox(config) => PolicyNodeJson::CountBox(CountBoxConfigJson {
                max_retries: config.max_retries,
                inner: config.inner,
            }),
            wit::PolicyNode::TimeBox(config) => PolicyNodeJson::TimeBox(TimeBoxConfigJson {
                limit: config.limit,
                inner: config.inner,
            }),
            wit::PolicyNode::ClampDelay(config) => PolicyNodeJson::ClampDelay(ClampConfigJson {
                min_delay: config.min_delay,
                max_delay: config.max_delay,
                inner: config.inner,
            }),
            wit::PolicyNode::AddDelay(config) => PolicyNodeJson::AddDelay(AddDelayConfigJson {
                delay: config.delay,
                inner: config.inner,
            }),
            wit::PolicyNode::Jitter(config) => PolicyNodeJson::Jitter(JitterConfigJson {
                factor: config.factor,
                inner: config.inner,
            }),
            wit::PolicyNode::FilteredOn(config) => PolicyNodeJson::FilteredOn(FilteredConfigJson {
                predicate: PredicateJson::from(&config.predicate),
                inner: config.inner,
            }),
            wit::PolicyNode::AndThen((left, right)) => PolicyNodeJson::AndThen((*left, *right)),
            wit::PolicyNode::PolicyUnion((left, right)) => {
                PolicyNodeJson::PolicyUnion((*left, *right))
            }
            wit::PolicyNode::PolicyIntersect((left, right)) => {
                PolicyNodeJson::PolicyIntersect((*left, *right))
            }
        }
    }
}

impl From<&wit::RetryPolicy> for PolicyJson {
    fn from(policy: &wit::RetryPolicy) -> Self {
        PolicyJson {
            nodes: policy.nodes.iter().map(PolicyNodeJson::from).collect(),
        }
    }
}

impl From<&wit::NamedRetryPolicy> for NamedPolicyJson {
    fn from(policy: &wit::NamedRetryPolicy) -> Self {
        NamedPolicyJson {
            name: policy.name.clone(),
            priority: policy.priority,
            predicate: PredicateJson::from(&policy.predicate),
            policy: PolicyJson::from(&policy.policy),
        }
    }
}

// ── Parsing: JSON → WIT ───────────────────────────────────────────────

/// A JSON string, integer or boolean as the `predicate-value` it mirrors.
fn value_from_json(value: &serde_json::Value) -> Result<wit::PredicateValue, String> {
    match value {
        serde_json::Value::String(text) => Ok(wit::PredicateValue::Text(text.clone())),
        serde_json::Value::Bool(flag) => Ok(wit::PredicateValue::Boolean(*flag)),
        serde_json::Value::Number(number) => number
            .as_i64()
            .map(wit::PredicateValue::Integer)
            .ok_or_else(|| format!("predicate value {number} is not an integer that fits i64")),
        other => Err(format!(
            "predicate value {other} must be a string, an integer or a boolean"
        )),
    }
}

fn comparison_from_json(comparison: &ComparisonJson) -> Result<wit::PropertyComparison, String> {
    Ok(wit::PropertyComparison {
        property_name: comparison.property_name.clone(),
        value: value_from_json(&comparison.value)?,
    })
}

fn set_check_from_json(check: &SetCheckJson) -> Result<wit::PropertySetCheck, String> {
    Ok(wit::PropertySetCheck {
        property_name: check.property_name.clone(),
        values: check
            .values
            .iter()
            .map(value_from_json)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn pattern_from_json(pattern: &PatternJson) -> wit::PropertyPattern {
    wit::PropertyPattern {
        property_name: pattern.property_name.clone(),
        pattern: pattern.pattern.clone(),
    }
}

impl TryFrom<&PredicateNodeJson> for wit::PredicateNode {
    type Error = String;

    fn try_from(node: &PredicateNodeJson) -> Result<Self, String> {
        Ok(match node {
            PredicateNodeJson::PropEq(comparison) => {
                wit::PredicateNode::PropEq(comparison_from_json(comparison)?)
            }
            PredicateNodeJson::PropNeq(comparison) => {
                wit::PredicateNode::PropNeq(comparison_from_json(comparison)?)
            }
            PredicateNodeJson::PropGt(comparison) => {
                wit::PredicateNode::PropGt(comparison_from_json(comparison)?)
            }
            PredicateNodeJson::PropGte(comparison) => {
                wit::PredicateNode::PropGte(comparison_from_json(comparison)?)
            }
            PredicateNodeJson::PropLt(comparison) => {
                wit::PredicateNode::PropLt(comparison_from_json(comparison)?)
            }
            PredicateNodeJson::PropLte(comparison) => {
                wit::PredicateNode::PropLte(comparison_from_json(comparison)?)
            }
            PredicateNodeJson::PropExists(property_name) => {
                wit::PredicateNode::PropExists(property_name.clone())
            }
            PredicateNodeJson::PropIn(check) => {
                wit::PredicateNode::PropIn(set_check_from_json(check)?)
            }
            PredicateNodeJson::PropMatches(pattern) => {
                wit::PredicateNode::PropMatches(pattern_from_json(pattern))
            }
            PredicateNodeJson::PropStartsWith(pattern) => {
                wit::PredicateNode::PropStartsWith(pattern_from_json(pattern))
            }
            PredicateNodeJson::PropContains(pattern) => {
                wit::PredicateNode::PropContains(pattern_from_json(pattern))
            }
            PredicateNodeJson::PredAnd((left, right)) => {
                wit::PredicateNode::PredAnd((*left, *right))
            }
            PredicateNodeJson::PredOr((left, right)) => wit::PredicateNode::PredOr((*left, *right)),
            PredicateNodeJson::PredNot(index) => wit::PredicateNode::PredNot(*index),
            PredicateNodeJson::PredTrue => wit::PredicateNode::PredTrue,
            PredicateNodeJson::PredFalse => wit::PredicateNode::PredFalse,
        })
    }
}

impl TryFrom<&PredicateJson> for wit::RetryPredicate {
    type Error = String;

    fn try_from(predicate: &PredicateJson) -> Result<Self, String> {
        Ok(wit::RetryPredicate {
            nodes: predicate
                .nodes
                .iter()
                .enumerate()
                .map(|(index, node)| {
                    wit::PredicateNode::try_from(node).map_err(|err| format!("node {index}: {err}"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<&PolicyNodeJson> for wit::PolicyNode {
    type Error = String;

    fn try_from(node: &PolicyNodeJson) -> Result<Self, String> {
        Ok(match node {
            PolicyNodeJson::Periodic(delay) => wit::PolicyNode::Periodic(*delay),
            PolicyNodeJson::Exponential(config) => {
                wit::PolicyNode::Exponential(wit::ExponentialConfig {
                    base_delay: config.base_delay,
                    factor: config.factor,
                })
            }
            PolicyNodeJson::Fibonacci(config) => wit::PolicyNode::Fibonacci(wit::FibonacciConfig {
                first: config.first,
                second: config.second,
            }),
            PolicyNodeJson::Immediate => wit::PolicyNode::Immediate,
            PolicyNodeJson::Never => wit::PolicyNode::Never,
            PolicyNodeJson::CountBox(config) => wit::PolicyNode::CountBox(wit::CountBoxConfig {
                max_retries: config.max_retries,
                inner: config.inner,
            }),
            PolicyNodeJson::TimeBox(config) => wit::PolicyNode::TimeBox(wit::TimeBoxConfig {
                limit: config.limit,
                inner: config.inner,
            }),
            PolicyNodeJson::ClampDelay(config) => wit::PolicyNode::ClampDelay(wit::ClampConfig {
                min_delay: config.min_delay,
                max_delay: config.max_delay,
                inner: config.inner,
            }),
            PolicyNodeJson::AddDelay(config) => wit::PolicyNode::AddDelay(wit::AddDelayConfig {
                delay: config.delay,
                inner: config.inner,
            }),
            PolicyNodeJson::Jitter(config) => wit::PolicyNode::Jitter(wit::JitterConfig {
                factor: config.factor,
                inner: config.inner,
            }),
            PolicyNodeJson::FilteredOn(config) => {
                wit::PolicyNode::FilteredOn(wit::FilteredConfig {
                    predicate: wit::RetryPredicate::try_from(&config.predicate)?,
                    inner: config.inner,
                })
            }
            PolicyNodeJson::AndThen((left, right)) => wit::PolicyNode::AndThen((*left, *right)),
            PolicyNodeJson::PolicyUnion((left, right)) => {
                wit::PolicyNode::PolicyUnion((*left, *right))
            }
            PolicyNodeJson::PolicyIntersect((left, right)) => {
                wit::PolicyNode::PolicyIntersect((*left, *right))
            }
        })
    }
}

impl TryFrom<&PolicyJson> for wit::RetryPolicy {
    type Error = String;

    fn try_from(policy: &PolicyJson) -> Result<Self, String> {
        Ok(wit::RetryPolicy {
            nodes: policy
                .nodes
                .iter()
                .enumerate()
                .map(|(index, node)| {
                    wit::PolicyNode::try_from(node).map_err(|err| format!("node {index}: {err}"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<&NamedPolicyJson> for wit::NamedRetryPolicy {
    type Error = String;

    fn try_from(policy: &NamedPolicyJson) -> Result<Self, String> {
        Ok(wit::NamedRetryPolicy {
            name: policy.name.clone(),
            priority: policy.priority,
            predicate: wit::RetryPredicate::try_from(&policy.predicate)
                .map_err(|err| format!("predicate {err}"))?,
            policy: wit::RetryPolicy::try_from(&policy.policy)
                .map_err(|err| format!("policy {err}"))?,
        })
    }
}

// ── The pieces the effects pass around ────────────────────────────────

/// A named policy — or a tree inside one — rendered as JSON text.
pub fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("rendering the retry JSON mirror cannot fail")
}

/// Every policy `get-retry-policies` returned, as one JSON array.
pub fn policies_to_json(policies: &[wit::NamedRetryPolicy]) -> String {
    let rendered: Vec<NamedPolicyJson> = policies.iter().map(NamedPolicyJson::from).collect();
    to_json(&rendered)
}

/// A named policy, as `setRetryPolicy!` takes it.
pub fn named_policy_from_json(json: &str) -> Result<wit::NamedRetryPolicy, String> {
    let parsed: NamedPolicyJson =
        serde_json::from_str(json).map_err(|err| format!("not a named retry policy: {err}"))?;
    wit::NamedRetryPolicy::try_from(&parsed)
}

/// A policy tree, as `nextRetryDelay!` takes it.
pub fn policy_from_json(json: &str) -> Result<wit::RetryPolicy, String> {
    let parsed: PolicyJson =
        serde_json::from_str(json).map_err(|err| format!("not a retry policy: {err}"))?;
    wit::RetryPolicy::try_from(&parsed)
}

/// The property map of a retry context: a JSON object whose values are strings,
/// integers or booleans.
pub fn properties_from_json(json: &str) -> Result<Vec<(String, wit::PredicateValue)>, String> {
    let parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(json)
        .map_err(|err| format!("retry properties must be a JSON object: {err}"))?;
    parsed
        .iter()
        .map(|(name, value)| {
            value_from_json(value)
                .map(|value| (name.clone(), value))
                .map_err(|err| format!("property {name:?}: {err}"))
        })
        .collect()
}

/// The context `nextRetryDelay!` evaluates its policy against.
pub struct DecisionContext {
    pub verb: String,
    pub noun_uri: String,
    pub properties: Vec<(String, wit::PredicateValue)>,
}

/// `{verb, nounUri, properties}`, the single argument `nextRetryDelay!` takes.
pub fn decision_context_from_json(json: &str) -> Result<DecisionContext, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct ContextJson {
        verb: String,
        noun_uri: String,
        #[serde(default)]
        properties: serde_json::Map<String, serde_json::Value>,
    }

    let parsed: ContextJson = serde_json::from_str(json)
        .map_err(|err| format!("retry context {{verb, nounUri, properties}} expected: {err}"))?;
    let properties = parsed
        .properties
        .iter()
        .map(|(name, value)| {
            value_from_json(value)
                .map(|value| (name.clone(), value))
                .map_err(|err| format!("property {name:?}: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DecisionContext {
        verb: parsed.verb,
        noun_uri: parsed.noun_uri,
        properties,
    })
}

/// The opaque continuation bytes of a retry sequence, as a JSON array of byte
/// values (`[0, 255, …]`).
fn state_to_json(state: &[u8]) -> String {
    serde_json::to_string(state).expect("a byte slice always serializes")
}

/// The continuation bytes the previous decision returned.
pub fn state_from_json(json: &str) -> Result<Vec<u8>, String> {
    let parsed: serde_json::Value = serde_json::from_str(json)
        .map_err(|err| format!("retry state must be a JSON array of bytes: {err}"))?;
    let items = parsed
        .as_array()
        .ok_or_else(|| String::from("retry state must be a JSON array of bytes"))?;
    items
        .iter()
        .map(|item| {
            item.as_u64()
                .and_then(|byte| u8::try_from(byte).ok())
                .ok_or_else(|| format!("retry state {item} is not a byte value"))
        })
        .collect()
}

/// `{state, delayNanos}`, with `null` for a delay of `none` (the sequence is
/// exhausted; a zero delay still permits another attempt).
pub fn decision_to_json(decision: &wit::RetryDecision) -> String {
    // The state is rendered through the same byte-array form `state_from_json`
    // reads, so a decision's state can be handed straight back in.
    let state: serde_json::Value =
        serde_json::from_str(&state_to_json(&decision.state)).expect("a byte array is JSON");
    serde_json::json!({
        "state": state,
        "delayNanos": decision.delay,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comparison(property_name: &str, value: wit::PredicateValue) -> wit::PropertyComparison {
        wit::PropertyComparison {
            property_name: property_name.to_string(),
            value,
        }
    }

    /// A policy the manifest could declare, with a predicate tree (an `and` of
    /// two comparisons) and a policy tree (a count box around a clamped
    /// periodic delay).
    fn sample_policy() -> wit::NamedRetryPolicy {
        wit::NamedRetryPolicy {
            name: String::from("e2e-retry"),
            priority: 100,
            predicate: wit::RetryPredicate {
                nodes: vec![
                    wit::PredicateNode::PredAnd((1, 2)),
                    wit::PredicateNode::PropEq(comparison(
                        "verb",
                        wit::PredicateValue::Text(String::from("send")),
                    )),
                    wit::PredicateNode::PropIn(wit::PropertySetCheck {
                        property_name: String::from("status"),
                        values: vec![
                            wit::PredicateValue::Integer(503),
                            wit::PredicateValue::Integer(504),
                        ],
                    }),
                ],
            },
            policy: wit::RetryPolicy {
                nodes: vec![
                    wit::PolicyNode::CountBox(wit::CountBoxConfig {
                        max_retries: 2,
                        inner: 1,
                    }),
                    wit::PolicyNode::ClampDelay(wit::ClampConfig {
                        min_delay: 100_000_000,
                        max_delay: 2_000_000_000,
                        inner: 2,
                    }),
                    wit::PolicyNode::Periodic(500_000_000),
                ],
            },
        }
    }

    #[test]
    fn test_named_policy_json_round_trip() {
        let policy = sample_policy();
        let json = to_json(&NamedPolicyJson::from(&policy));

        // The rendering is the WIT shape: camelCase tags, indices as numbers,
        // durations in nanoseconds, predicate values as plain JSON values.
        assert_eq!(
            json,
            concat!(
                r#"{"name":"e2e-retry","priority":100,"predicate":{"nodes":["#,
                r#"{"predAnd":[1,2]},"#,
                r#"{"propEq":{"propertyName":"verb","value":"send"}},"#,
                r#"{"propIn":{"propertyName":"status","values":[503,504]}}]},"#,
                r#""policy":{"nodes":["#,
                r#"{"countBox":{"maxRetries":2,"inner":1}},"#,
                r#"{"clampDelay":{"minDelay":100000000,"maxDelay":2000000000,"inner":2}},"#,
                r#"{"periodic":500000000}]}}"#,
            )
        );

        // And what it printed parses back into the same tree.
        let parsed = named_policy_from_json(&json).expect("the rendering must parse back");
        assert_eq!(parsed.name, "e2e-retry");
        assert_eq!(parsed.priority, 100);
        assert_eq!(parsed.predicate.nodes.len(), 3);
        assert_eq!(parsed.policy.nodes.len(), 3);
        assert_eq!(
            to_json(&NamedPolicyJson::from(&parsed)),
            json,
            "the parsed tree must render back to the same JSON"
        );
    }

    #[test]
    fn test_unit_variants_are_bare_strings() {
        let policy = wit::RetryPolicy {
            nodes: vec![wit::PolicyNode::Immediate, wit::PolicyNode::Never],
        };
        let json = to_json(&PolicyJson::from(&policy));
        assert_eq!(json, r#"{"nodes":["immediate","never"]}"#);
        assert_eq!(
            to_json(&PolicyJson::from(
                &policy_from_json(&json).expect("parses back")
            )),
            json
        );

        let predicate = wit::RetryPredicate {
            nodes: vec![
                wit::PredicateNode::PredTrue,
                wit::PredicateNode::PredFalse,
                wit::PredicateNode::PredNot(0),
                wit::PredicateNode::PropExists(String::from("retry-count")),
            ],
        };
        let json = to_json(&PredicateJson::from(&predicate));
        assert_eq!(
            json,
            r#"{"nodes":["predTrue","predFalse",{"predNot":0},{"propExists":"retry-count"}]}"#
        );
        let parsed =
            wit::RetryPredicate::try_from(&serde_json::from_str::<PredicateJson>(&json).unwrap())
                .expect("parses back");
        assert_eq!(parsed.nodes.len(), 4);
        assert_eq!(to_json(&PredicateJson::from(&parsed)), json);
    }

    /// `filtered-on` carries a predicate tree inline, which has to survive one
    /// more level of nesting than the other nodes.
    #[test]
    fn test_filtered_on_embeds_a_predicate_tree() {
        let policy = wit::RetryPolicy {
            nodes: vec![wit::PolicyNode::FilteredOn(wit::FilteredConfig {
                predicate: wit::RetryPredicate {
                    nodes: vec![wit::PredicateNode::PropEq(comparison(
                        "status-code",
                        wit::PredicateValue::Integer(503),
                    ))],
                },
                inner: 1,
            })],
        };
        let json = to_json(&PolicyJson::from(&policy));
        assert_eq!(
            json,
            concat!(
                r#"{"nodes":[{"filteredOn":{"predicate":{"nodes":["#,
                r#"{"propEq":{"propertyName":"status-code","value":503}}]},"#,
                r#""inner":1}}]}"#,
            )
        );
        let parsed = policy_from_json(&json).expect("parses back");
        match &parsed.nodes[0] {
            wit::PolicyNode::FilteredOn(config) => assert_eq!(config.inner, 1),
            other => panic!("expected a filtered-on node, got {other:?}"),
        }
    }

    #[test]
    fn test_policy_parse_errors() {
        // The tag has to be a WIT variant name.
        let err = policy_from_json(r#"{"nodes":[{"clamp":{}}]}"#).expect_err("unknown tag");
        assert!(err.contains("clamp"), "unexpected error: {err}");

        // A predicate value has to be a string, integer or boolean.
        let err = named_policy_from_json(
            r#"{"name":"p","priority":1,"predicate":{"nodes":[{"propEq":{"propertyName":"verb","value":{}}}]},"policy":{"nodes":["immediate"]}}"#,
        )
        .expect_err("object-valued comparison");
        assert!(err.contains("predicate node 0"), "unexpected error: {err}");

        // A tree with no nodes has no root.
        assert!(named_policy_from_json("not json").is_err());
    }

    #[test]
    fn test_properties_from_json() {
        let properties = properties_from_json(r#"{"verb":"send","attempt":3,"retryable":true}"#)
            .expect("properties object");
        let mut names: Vec<&str> = properties.iter().map(|(name, _)| name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["attempt", "retryable", "verb"]);
        assert!(properties
            .iter()
            .any(|(name, value)| name == "attempt"
                && matches!(value, wit::PredicateValue::Integer(3))));

        // A list of pairs, a bare value or a float are all rejected with an
        // explanation.
        assert!(properties_from_json(r#"[["verb","send"]]"#).is_err());
        assert!(properties_from_json("3").is_err());
        let err = properties_from_json(r#"{"ratio":1.5}"#).expect_err("fractional value");
        assert!(err.contains("ratio"), "unexpected error: {err}");
        assert!(properties_from_json("{}")
            .expect("no properties")
            .is_empty());
    }

    #[test]
    fn test_decision_context_from_json() {
        let context = decision_context_from_json(
            r#"{"verb":"send","nounUri":"quota://e2e","properties":{"attempt":1}}"#,
        )
        .expect("context object");
        assert_eq!(context.verb, "send");
        assert_eq!(context.noun_uri, "quota://e2e");
        assert_eq!(context.properties.len(), 1);

        // `properties` is optional, and an unknown field is a typo, not a
        // silent default.
        let context = decision_context_from_json(r#"{"verb":"send","nounUri":"quota://e2e"}"#)
            .expect("context without properties");
        assert!(context.properties.is_empty());
        assert!(decision_context_from_json(r#"{"verb":"send","noun":"quota://e2e"}"#).is_err());
    }

    #[test]
    fn test_state_and_decision_json() {
        assert_eq!(state_to_json(&[]), "[]");
        assert_eq!(state_to_json(&[0, 7, 255]), "[0,7,255]");
        assert_eq!(
            state_from_json("[0,7,255]").expect("bytes"),
            vec![0, 7, 255]
        );
        assert!(state_from_json("[0,256]").is_err());
        assert!(state_from_json("[0,-1]").is_err());
        assert!(state_from_json(r#""a""#).is_err());

        let decision = wit::RetryDecision {
            state: vec![1, 2],
            delay: Some(100_000_000),
        };
        assert_eq!(
            decision_to_json(&decision),
            r#"{"delayNanos":100000000,"state":[1,2]}"#
        );

        // `none` is how an exhausted policy reports "stop".
        let exhausted = wit::RetryDecision {
            state: vec![],
            delay: None,
        };
        assert_eq!(
            decision_to_json(&exhausted),
            r#"{"delayNanos":null,"state":[]}"#
        );
    }
}
