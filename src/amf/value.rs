//! AMF value types
//!
//! Both AMF0 and AMF3 share a common value representation. This enum
//! provides a unified type that can be serialized to either format.

use std::collections::HashMap;

/// Unified AMF value representation
///
/// This enum represents all value types supported by AMF0 and AMF3.
/// Some types (like ByteArray, Dictionary) are AMF3-only but included
/// for completeness.
#[derive(Debug, Clone, PartialEq)]
pub enum AmfValue {
    /// Null value (AMF0: 0x05, AMF3: 0x01)
    Null,

    /// Undefined value (AMF0: 0x06, AMF3: 0x00)
    Undefined,

    /// Boolean value (AMF0: 0x01, AMF3: 0x02/0x03)
    Boolean(bool),

    /// IEEE 754 double-precision floating point (AMF0: 0x00, AMF3: 0x05)
    Number(f64),

    /// UTF-8 string (AMF0: 0x02, AMF3: 0x06)
    String(String),

    /// Ordered array with optional associative portion
    /// In AMF0 this is either StrictArray (0x0A) or ECMAArray (0x08)
    /// In AMF3 this is Array (0x09)
    Array(Vec<AmfValue>),

    /// Key-value object (AMF0: 0x03, AMF3: 0x0A)
    /// Keys are always strings in AMF
    Object(HashMap<String, AmfValue>),

    /// Typed object with class name
    TypedObject {
        class_name: String,
        properties: HashMap<String, AmfValue>,
    },

    /// Date value as milliseconds since Unix epoch
    /// (AMF0: 0x0B, AMF3: 0x08)
    Date(f64),

    /// XML document (AMF0: 0x0F, AMF3: 0x07/0x0B)
    Xml(String),

    /// Raw byte array (AMF3 only: 0x0C)
    ByteArray(Vec<u8>),

    /// Integer (AMF3 only: 0x04, 29-bit signed)
    Integer(i32),

    /// ECMA Array - associative array with dense and sparse parts
    /// Stored as (dense_length, properties)
    EcmaArray(HashMap<String, AmfValue>),
}

impl AmfValue {
    /// Try to get this value as a string reference
    pub fn as_str(&self) -> Option<&str> {
        match self {
            AmfValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get this value as a number
    pub fn as_number(&self) -> Option<f64> {
        match self {
            AmfValue::Number(n) => Some(*n),
            AmfValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get this value as a boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AmfValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get this value as an object reference
    pub fn as_object(&self) -> Option<&HashMap<String, AmfValue>> {
        match self {
            AmfValue::Object(m) => Some(m),
            AmfValue::EcmaArray(m) => Some(m),
            AmfValue::TypedObject { properties, .. } => Some(properties),
            _ => None,
        }
    }

    /// Try to get this value as a mutable object reference
    pub fn as_object_mut(&mut self) -> Option<&mut HashMap<String, AmfValue>> {
        match self {
            AmfValue::Object(m) => Some(m),
            AmfValue::EcmaArray(m) => Some(m),
            AmfValue::TypedObject { properties, .. } => Some(properties),
            _ => None,
        }
    }

    /// Try to get this value as an array reference
    pub fn as_array(&self) -> Option<&Vec<AmfValue>> {
        match self {
            AmfValue::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Check if this value is null or undefined
    pub fn is_null_or_undefined(&self) -> bool {
        matches!(self, AmfValue::Null | AmfValue::Undefined)
    }

    /// Get a property from an object value
    pub fn get(&self, key: &str) -> Option<&AmfValue> {
        self.as_object()?.get(key)
    }

    /// Get a string property from an object value
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    /// Get a number property from an object value
    pub fn get_number(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_number()
    }
}

impl Default for AmfValue {
    fn default() -> Self {
        AmfValue::Null
    }
}

impl From<bool> for AmfValue {
    fn from(v: bool) -> Self {
        AmfValue::Boolean(v)
    }
}

impl From<f64> for AmfValue {
    fn from(v: f64) -> Self {
        AmfValue::Number(v)
    }
}

impl From<i32> for AmfValue {
    fn from(v: i32) -> Self {
        AmfValue::Number(v as f64)
    }
}

impl From<u32> for AmfValue {
    fn from(v: u32) -> Self {
        AmfValue::Number(v as f64)
    }
}

impl From<String> for AmfValue {
    fn from(v: String) -> Self {
        AmfValue::String(v)
    }
}

impl From<&str> for AmfValue {
    fn from(v: &str) -> Self {
        AmfValue::String(v.to_string())
    }
}

impl<V: Into<AmfValue>> From<Vec<V>> for AmfValue {
    fn from(v: Vec<V>) -> Self {
        AmfValue::Array(v.into_iter().map(|x| x.into()).collect())
    }
}

impl<V: Into<AmfValue>> From<HashMap<String, V>> for AmfValue {
    fn from(v: HashMap<String, V>) -> Self {
        AmfValue::Object(v.into_iter().map(|(k, v)| (k, v.into())).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_accessors() {
        let s = AmfValue::String("test".into());
        assert_eq!(s.as_str(), Some("test"));
        assert_eq!(s.as_number(), None);

        let n = AmfValue::Number(42.0);
        assert_eq!(n.as_number(), Some(42.0));
        assert_eq!(n.as_str(), None);

        let mut obj = HashMap::new();
        obj.insert("key".to_string(), AmfValue::String("value".into()));
        let o = AmfValue::Object(obj);
        assert_eq!(o.get_string("key"), Some("value"));
    }

    #[test]
    fn test_from_conversions() {
        let v: AmfValue = "test".into();
        assert!(matches!(v, AmfValue::String(_)));

        let v: AmfValue = 42.0.into();
        assert!(matches!(v, AmfValue::Number(_)));

        let v: AmfValue = true.into();
        assert!(matches!(v, AmfValue::Boolean(true)));
    }

    #[test]
    fn test_as_bool() {
        assert_eq!(AmfValue::Boolean(true).as_bool(), Some(true));
        assert_eq!(AmfValue::Boolean(false).as_bool(), Some(false));
        assert_eq!(AmfValue::Number(1.0).as_bool(), None);
        assert_eq!(AmfValue::Null.as_bool(), None);
    }

    #[test]
    fn test_as_array() {
        let arr = AmfValue::Array(vec![AmfValue::Number(1.0), AmfValue::Number(2.0)]);
        assert!(arr.as_array().is_some());
        assert_eq!(arr.as_array().unwrap().len(), 2);

        assert!(AmfValue::Null.as_array().is_none());
        assert!(AmfValue::Object(HashMap::new()).as_array().is_none());
    }

    #[test]
    fn test_as_object_mut() {
        let mut obj = AmfValue::Object(HashMap::new());
        if let Some(map) = obj.as_object_mut() {
            map.insert("key".to_string(), AmfValue::String("value".into()));
        }

        assert_eq!(obj.get_string("key"), Some("value"));
    }

    #[test]
    fn test_is_null_or_undefined() {
        assert!(AmfValue::Null.is_null_or_undefined());
        assert!(AmfValue::Undefined.is_null_or_undefined());
        assert!(!AmfValue::Boolean(false).is_null_or_undefined());
        assert!(!AmfValue::Number(0.0).is_null_or_undefined());
        assert!(!AmfValue::String(String::new()).is_null_or_undefined());
    }

    #[test]
    fn test_get_number() {
        let mut obj = HashMap::new();
        obj.insert("count".to_string(), AmfValue::Number(42.0));
        obj.insert("name".to_string(), AmfValue::String("test".into()));
        let amf = AmfValue::Object(obj);

        assert_eq!(amf.get_number("count"), Some(42.0));
        assert_eq!(amf.get_number("name"), None);
        assert_eq!(amf.get_number("missing"), None);
    }

    #[test]
    fn test_integer_as_number() {
        let integer = AmfValue::Integer(100);
        assert_eq!(integer.as_number(), Some(100.0));
    }

    #[test]
    fn test_default_value() {
        let default = AmfValue::default();
        assert_eq!(default, AmfValue::Null);
    }

    #[test]
    fn test_from_i32() {
        let v: AmfValue = 42i32.into();
        assert_eq!(v, AmfValue::Number(42.0));
    }

    #[test]
    fn test_from_u32() {
        let v: AmfValue = 1000u32.into();
        assert_eq!(v, AmfValue::Number(1000.0));
    }

    #[test]
    fn test_from_string_owned() {
        let s = String::from("owned");
        let v: AmfValue = s.into();
        assert_eq!(v.as_str(), Some("owned"));
    }

    #[test]
    fn test_from_vec() {
        let vec: Vec<f64> = vec![1.0, 2.0, 3.0];
        let v: AmfValue = vec.into();
        if let AmfValue::Array(arr) = v {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected Array");
        }
    }

    #[test]
    fn test_from_hashmap() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), 1.0f64);
        map.insert("b".to_string(), 2.0f64);

        let v: AmfValue = map.into();
        if let AmfValue::Object(obj) = v {
            assert_eq!(obj.len(), 2);
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_as_object_with_typed_object() {
        let mut props = HashMap::new();
        props.insert("x".to_string(), AmfValue::Number(10.0));

        let typed = AmfValue::TypedObject {
            class_name: "Point".to_string(),
            properties: props,
        };

        // as_object should work on TypedObject
        assert!(typed.as_object().is_some());
        assert_eq!(typed.get_number("x"), Some(10.0));
    }

    #[test]
    fn test_as_object_with_ecma_array() {
        let mut props = HashMap::new();
        props.insert("key".to_string(), AmfValue::String("value".into()));

        let ecma = AmfValue::EcmaArray(props);

        // as_object should work on EcmaArray
        assert!(ecma.as_object().is_some());
        assert_eq!(ecma.get_string("key"), Some("value"));
    }

    #[test]
    fn test_get_on_non_object() {
        assert!(AmfValue::Null.get("key").is_none());
        assert!(AmfValue::Number(42.0).get("key").is_none());
        assert!(AmfValue::Array(vec![]).get("0").is_none());
    }

    #[test]
    fn test_amf_value_clone() {
        let original = AmfValue::Object({
            let mut m = HashMap::new();
            m.insert(
                "nested".to_string(),
                AmfValue::Array(vec![AmfValue::Number(1.0), AmfValue::String("test".into())]),
            );
            m
        });

        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_amf_value_partial_eq() {
        assert_eq!(AmfValue::Null, AmfValue::Null);
        assert_ne!(AmfValue::Null, AmfValue::Undefined);
        assert_eq!(AmfValue::Number(1.0), AmfValue::Number(1.0));
        assert_ne!(AmfValue::Number(1.0), AmfValue::Number(2.0));
        assert_eq!(
            AmfValue::String("test".into()),
            AmfValue::String("test".into())
        );
    }
}
