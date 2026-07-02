//! Typed property values.
//!
//! The shared value-type vocabulary (`ValueType`) plus the deterministic
//! scalar codec (`parse_scalar` / `format_scalar`) that the CRDT materializer
//! relies on: the same stored scalar must always render to the same bytes, or
//! two replicas with equal state diverge on disk. Coerce-and-keep — never
//! reject — per the failure policy (CRDT is the source of truth).

#[cfg(test)]
use ts_rs::TS;

/// The canonical value-type vocabulary — THE one source of truth for what a
/// property's `value_type` string may mean, across Rust, web (`PropertyType`
/// in `property-registry.ts`), and iOS (`PropertyType` in
/// `PropertyRegistry.swift`). `crate::types::PropertyDef::value_type` stores
/// this vocabulary's string form (`as_str()`), parsed back via [`parse`] —
/// there is no second, independently-validated type vocabulary in Rust.
/// Unknown strings degrade to `Text` (coerce-and-keep: validation is a view,
/// never a gate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    Text,
    Number,
    Date,
    DateTime,
    Checkbox,
    Url,
    Select,
    MultiSelect,
    Node,
    Email,
    Phone,
    Object,
}

impl ValueType {
    /// Parse a `value_type` string; an unrecognized value degrades to `Text`.
    pub fn parse(s: &str) -> ValueType {
        match s {
            "number" => ValueType::Number,
            "date" => ValueType::Date,
            "datetime" => ValueType::DateTime,
            "checkbox" => ValueType::Checkbox,
            "url" => ValueType::Url,
            "select" => ValueType::Select,
            "multiselect" => ValueType::MultiSelect,
            "node" => ValueType::Node,
            "email" => ValueType::Email,
            "phone" => ValueType::Phone,
            "object" => ValueType::Object,
            _ => ValueType::Text,
        }
    }

    /// The canonical lowercase string form (round-trips with [`ValueType::parse`]).
    pub fn as_str(self) -> &'static str {
        match self {
            ValueType::Text => "text",
            ValueType::Number => "number",
            ValueType::Date => "date",
            ValueType::DateTime => "datetime",
            ValueType::Checkbox => "checkbox",
            ValueType::Url => "url",
            ValueType::Select => "select",
            ValueType::MultiSelect => "multiselect",
            ValueType::Node => "node",
            ValueType::Email => "email",
            ValueType::Phone => "phone",
            ValueType::Object => "object",
        }
    }
}

/// A single stored scalar property value, mirroring the Loro primitive forms.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PropScalar {
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

/// Coerce a raw user string into a typed scalar per its value-type. Never
/// fails: an uncoercible value is kept as `Text` (coerce-and-keep).
pub fn parse_scalar(value_type: ValueType, raw: &str) -> PropScalar {
    match value_type {
        ValueType::Checkbox => PropScalar::Bool(raw.eq_ignore_ascii_case("true")),
        ValueType::Number => {
            if let Ok(i) = raw.parse::<i64>() {
                PropScalar::Int(i)
            } else if let Ok(f) = raw.parse::<f64>() {
                PropScalar::Float(f)
            } else {
                PropScalar::Text(raw.to_string())
            }
        }
        _ => PropScalar::Text(raw.to_string()),
    }
}

/// Render a stored scalar to its canonical string form. Determinism-critical:
/// the same scalar must always produce the same bytes, or replicas with equal
/// CRDT state diverge on disk. `f64`'s `Display` gives the shortest round-trip
/// representation (`3.0` -> `"3"`, `3.50` -> `"3.5"`) with no exponent.
pub fn format_scalar(value: &PropScalar) -> String {
    match value {
        PropScalar::Text(s) => s.clone(),
        PropScalar::Int(i) => i.to_string(),
        PropScalar::Float(f) => f.to_string(),
        PropScalar::Bool(b) => if *b { "true" } else { "false" }.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_type_parses_known_strings() {
        assert_eq!(ValueType::parse("select"), ValueType::Select);
        assert_eq!(ValueType::parse("multiselect"), ValueType::MultiSelect);
        assert_eq!(ValueType::parse("datetime"), ValueType::DateTime);
        assert_eq!(ValueType::parse("node"), ValueType::Node);
        assert_eq!(ValueType::parse("email"), ValueType::Email);
        assert_eq!(ValueType::parse("phone"), ValueType::Phone);
        assert_eq!(ValueType::parse("object"), ValueType::Object);
    }

    #[test]
    fn value_type_degrades_unknown_to_text() {
        // Coerce-and-keep (locked 2026-06-05): an unrecognized `value_type`
        // string — including one from a NEWER vocabulary this Rust build
        // doesn't know about yet — silently degrades to `Text` rather than
        // erroring, and round-trips through `as_str()` as `"text"`.
        assert_eq!(ValueType::parse("totally-bogus"), ValueType::Text);
        assert_eq!(ValueType::parse("totally-bogus").as_str(), "text");
    }

    #[test]
    fn value_type_round_trips_via_as_str() {
        for s in [
            "text",
            "number",
            "date",
            "datetime",
            "checkbox",
            "url",
            "select",
            "multiselect",
            "node",
            "email",
            "phone",
            "object",
        ] {
            assert_eq!(ValueType::parse(s).as_str(), s);
        }
    }

    #[test]
    fn checkbox_scalar_formats_canonically() {
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Checkbox, "true")),
            "true"
        );
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Checkbox, "TRUE")),
            "true"
        );
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Checkbox, "false")),
            "false"
        );
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Checkbox, "anything-else")),
            "false"
        );
    }

    #[test]
    fn number_scalar_drops_trailing_zeros() {
        assert_eq!(format_scalar(&parse_scalar(ValueType::Number, "3")), "3");
        assert_eq!(format_scalar(&parse_scalar(ValueType::Number, "3.0")), "3");
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Number, "3.50")),
            "3.5"
        );
        // coerce-and-keep: a non-numeric value is preserved verbatim, never dropped
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Number, "not-a-number")),
            "not-a-number"
        );
    }

    #[test]
    fn text_like_scalars_pass_through_verbatim() {
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Date, "2026-06-10")),
            "2026-06-10"
        );
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Select, "doing")),
            "doing"
        );
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Email, "a@b.com")),
            "a@b.com"
        );
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Phone, "555-0100")),
            "555-0100"
        );
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Object, "some-note-id")),
            "some-note-id"
        );
        assert_eq!(
            format_scalar(&parse_scalar(ValueType::Node, "alice")),
            "alice"
        );
    }
}
