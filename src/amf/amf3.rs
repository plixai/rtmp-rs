//! AMF3 encoder and decoder
//!
//! AMF3 is the ActionScript 3.0 serialization format. It's more efficient
//! than AMF0 due to better string/object references and a native integer type.
//!
//! Most RTMP implementations use AMF0 for commands. AMF3 support is included
//! for completeness and for handling avmplus-object markers (0x11) in AMF0 streams.
//!
//! Type Markers:
//! ```text
//! 0x00 - Undefined
//! 0x01 - Null
//! 0x02 - Boolean false
//! 0x03 - Boolean true
//! 0x04 - Integer (29-bit signed)
//! 0x05 - Double
//! 0x06 - String
//! 0x07 - XML Document (legacy)
//! 0x08 - Date
//! 0x09 - Array
//! 0x0A - Object
//! 0x0B - XML
//! 0x0C - ByteArray
//! ```

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;

use crate::error::AmfError;
use super::value::AmfValue;

// AMF3 type markers
const MARKER_UNDEFINED: u8 = 0x00;
const MARKER_NULL: u8 = 0x01;
const MARKER_FALSE: u8 = 0x02;
const MARKER_TRUE: u8 = 0x03;
const MARKER_INTEGER: u8 = 0x04;
const MARKER_DOUBLE: u8 = 0x05;
const MARKER_STRING: u8 = 0x06;
const MARKER_XML_DOC: u8 = 0x07;
const MARKER_DATE: u8 = 0x08;
const MARKER_ARRAY: u8 = 0x09;
const MARKER_OBJECT: u8 = 0x0A;
const MARKER_XML: u8 = 0x0B;
const MARKER_BYTE_ARRAY: u8 = 0x0C;

/// Maximum nesting depth
const MAX_NESTING_DEPTH: usize = 64;

/// AMF3 29-bit integer bounds
const AMF3_INT_MAX: i32 = 0x0FFFFFFF;
const AMF3_INT_MIN: i32 = -0x10000000;

/// AMF3 decoder with reference tables
pub struct Amf3Decoder {
    /// String reference table
    string_refs: Vec<String>,
    /// Object reference table
    object_refs: Vec<AmfValue>,
    /// Trait reference table (class definitions)
    trait_refs: Vec<TraitDef>,
    /// Enable lenient parsing
    lenient: bool,
    /// Current nesting depth
    depth: usize,
}

/// Trait definition for typed objects
#[derive(Clone, Debug)]
struct TraitDef {
    class_name: String,
    is_dynamic: bool,
    properties: Vec<String>,
}

impl Amf3Decoder {
    /// Create a new decoder
    pub fn new() -> Self {
        Self {
            string_refs: Vec::new(),
            object_refs: Vec::new(),
            trait_refs: Vec::new(),
            lenient: true,
            depth: 0,
        }
    }

    /// Reset decoder state
    pub fn reset(&mut self) {
        self.string_refs.clear();
        self.object_refs.clear();
        self.trait_refs.clear();
        self.depth = 0;
    }

    /// Decode a single AMF3 value
    pub fn decode(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        if buf.is_empty() {
            return Err(AmfError::UnexpectedEof);
        }

        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            return Err(AmfError::NestingTooDeep);
        }

        let marker = buf.get_u8();
        let result = self.decode_value(marker, buf);
        self.depth -= 1;
        result
    }

    fn decode_value(&mut self, marker: u8, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        match marker {
            MARKER_UNDEFINED => Ok(AmfValue::Undefined),
            MARKER_NULL => Ok(AmfValue::Null),
            MARKER_FALSE => Ok(AmfValue::Boolean(false)),
            MARKER_TRUE => Ok(AmfValue::Boolean(true)),
            MARKER_INTEGER => self.decode_integer(buf),
            MARKER_DOUBLE => self.decode_double(buf),
            MARKER_STRING => self.decode_string(buf),
            MARKER_DATE => self.decode_date(buf),
            MARKER_ARRAY => self.decode_array(buf),
            MARKER_OBJECT => self.decode_object(buf),
            MARKER_BYTE_ARRAY => self.decode_byte_array(buf),
            MARKER_XML | MARKER_XML_DOC => self.decode_xml(buf),
            _ => {
                if self.lenient {
                    Ok(AmfValue::Undefined)
                } else {
                    Err(AmfError::UnknownMarker(marker))
                }
            }
        }
    }

    fn decode_integer(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        let value = self.read_u29(buf)?;
        // Sign-extend from 29 bits
        let signed = if value & 0x10000000 != 0 {
            (value as i32) | !0x1FFFFFFF
        } else {
            value as i32
        };
        Ok(AmfValue::Integer(signed))
    }

    fn decode_double(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        if buf.remaining() < 8 {
            return Err(AmfError::UnexpectedEof);
        }
        Ok(AmfValue::Number(buf.get_f64()))
    }

    fn decode_string(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        let s = self.read_string(buf)?;
        Ok(AmfValue::String(s))
    }

    fn decode_date(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        let header = self.read_u29(buf)?;

        if header & 1 == 0 {
            // Reference
            let idx = (header >> 1) as usize;
            if idx >= self.object_refs.len() {
                return Err(AmfError::InvalidReference(idx as u16));
            }
            return Ok(self.object_refs[idx].clone());
        }

        if buf.remaining() < 8 {
            return Err(AmfError::UnexpectedEof);
        }

        let timestamp = buf.get_f64();
        let value = AmfValue::Date(timestamp);
        self.object_refs.push(value.clone());
        Ok(value)
    }

    fn decode_array(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        let header = self.read_u29(buf)?;

        if header & 1 == 0 {
            // Reference
            let idx = (header >> 1) as usize;
            if idx >= self.object_refs.len() {
                return Err(AmfError::InvalidReference(idx as u16));
            }
            return Ok(self.object_refs[idx].clone());
        }

        let dense_count = (header >> 1) as usize;

        // Placeholder for self-reference
        let arr_idx = self.object_refs.len();
        self.object_refs.push(AmfValue::Null);

        // Read associative portion (key-value pairs until empty string)
        let mut assoc = HashMap::new();
        loop {
            let key = self.read_string(buf)?;
            if key.is_empty() {
                break;
            }
            let value = self.decode(buf)?;
            assoc.insert(key, value);
        }

        // Read dense portion
        let mut dense = Vec::with_capacity(dense_count.min(1024));
        for _ in 0..dense_count {
            dense.push(self.decode(buf)?);
        }

        let value = if assoc.is_empty() {
            AmfValue::Array(dense)
        } else {
            // Mixed array - store as ECMA array with dense values as string keys
            for (i, v) in dense.into_iter().enumerate() {
                assoc.insert(i.to_string(), v);
            }
            AmfValue::EcmaArray(assoc)
        };

        self.object_refs[arr_idx] = value.clone();
        Ok(value)
    }

    fn decode_object(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        let header = self.read_u29(buf)?;

        if header & 1 == 0 {
            // Object reference
            let idx = (header >> 1) as usize;
            if idx >= self.object_refs.len() {
                return Err(AmfError::InvalidReference(idx as u16));
            }
            return Ok(self.object_refs[idx].clone());
        }

        // Placeholder for self-reference
        let obj_idx = self.object_refs.len();
        self.object_refs.push(AmfValue::Null);

        let trait_def = if header & 2 == 0 {
            // Trait reference
            let idx = (header >> 2) as usize;
            if idx >= self.trait_refs.len() {
                return Err(AmfError::InvalidReference(idx as u16));
            }
            self.trait_refs[idx].clone()
        } else {
            // Inline trait
            let is_dynamic = (header & 8) != 0;
            let sealed_count = (header >> 4) as usize;

            let class_name = self.read_string(buf)?;

            let mut properties = Vec::with_capacity(sealed_count);
            for _ in 0..sealed_count {
                properties.push(self.read_string(buf)?);
            }

            let trait_def = TraitDef {
                class_name,
                is_dynamic,
                properties,
            };
            self.trait_refs.push(trait_def.clone());
            trait_def
        };

        let mut props = HashMap::new();

        // Read sealed properties
        for prop_name in &trait_def.properties {
            let value = self.decode(buf)?;
            props.insert(prop_name.clone(), value);
        }

        // Read dynamic properties
        if trait_def.is_dynamic {
            loop {
                let key = self.read_string(buf)?;
                if key.is_empty() {
                    break;
                }
                let value = self.decode(buf)?;
                props.insert(key, value);
            }
        }

        let value = if trait_def.class_name.is_empty() {
            AmfValue::Object(props)
        } else {
            AmfValue::TypedObject {
                class_name: trait_def.class_name,
                properties: props,
            }
        };

        self.object_refs[obj_idx] = value.clone();
        Ok(value)
    }

    fn decode_byte_array(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        let header = self.read_u29(buf)?;

        if header & 1 == 0 {
            let idx = (header >> 1) as usize;
            if idx >= self.object_refs.len() {
                return Err(AmfError::InvalidReference(idx as u16));
            }
            return Ok(self.object_refs[idx].clone());
        }

        let len = (header >> 1) as usize;
        if buf.remaining() < len {
            return Err(AmfError::UnexpectedEof);
        }

        let data = buf.copy_to_bytes(len).to_vec();
        let value = AmfValue::ByteArray(data);
        self.object_refs.push(value.clone());
        Ok(value)
    }

    fn decode_xml(&mut self, buf: &mut Bytes) -> Result<AmfValue, AmfError> {
        let header = self.read_u29(buf)?;

        if header & 1 == 0 {
            let idx = (header >> 1) as usize;
            if idx >= self.object_refs.len() {
                return Err(AmfError::InvalidReference(idx as u16));
            }
            return Ok(self.object_refs[idx].clone());
        }

        let len = (header >> 1) as usize;
        if buf.remaining() < len {
            return Err(AmfError::UnexpectedEof);
        }

        let bytes = buf.copy_to_bytes(len);
        let s = String::from_utf8(bytes.to_vec()).map_err(|_| AmfError::InvalidUtf8)?;
        let value = AmfValue::Xml(s);
        self.object_refs.push(value.clone());
        Ok(value)
    }

    /// Read AMF3 U29 variable-length integer
    fn read_u29(&mut self, buf: &mut Bytes) -> Result<u32, AmfError> {
        let mut value: u32 = 0;

        for i in 0..4 {
            if buf.is_empty() {
                return Err(AmfError::UnexpectedEof);
            }

            let byte = buf.get_u8();

            if i < 3 {
                value = (value << 7) | ((byte & 0x7F) as u32);
                if byte & 0x80 == 0 {
                    return Ok(value);
                }
            } else {
                // Fourth byte uses all 8 bits
                value = (value << 8) | (byte as u32);
                return Ok(value);
            }
        }

        Ok(value)
    }

    /// Read AMF3 string (with reference handling)
    fn read_string(&mut self, buf: &mut Bytes) -> Result<String, AmfError> {
        let header = self.read_u29(buf)?;

        if header & 1 == 0 {
            // Reference
            let idx = (header >> 1) as usize;
            if idx >= self.string_refs.len() {
                return Err(AmfError::InvalidReference(idx as u16));
            }
            return Ok(self.string_refs[idx].clone());
        }

        let len = (header >> 1) as usize;
        if len == 0 {
            return Ok(String::new());
        }

        if buf.remaining() < len {
            return Err(AmfError::UnexpectedEof);
        }

        let bytes = buf.copy_to_bytes(len);
        let s = String::from_utf8(bytes.to_vec()).map_err(|_| AmfError::InvalidUtf8)?;

        // Only non-empty strings go into reference table
        self.string_refs.push(s.clone());
        Ok(s)
    }
}

impl Default for Amf3Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// AMF3 encoder
pub struct Amf3Encoder {
    buf: BytesMut,
    string_refs: HashMap<String, usize>,
}

impl Amf3Encoder {
    /// Create a new encoder
    pub fn new() -> Self {
        Self {
            buf: BytesMut::with_capacity(256),
            string_refs: HashMap::new(),
        }
    }

    /// Get encoded bytes and reset
    pub fn finish(&mut self) -> Bytes {
        self.string_refs.clear();
        self.buf.split().freeze()
    }

    /// Encode a single AMF3 value
    pub fn encode(&mut self, value: &AmfValue) {
        match value {
            AmfValue::Undefined => self.buf.put_u8(MARKER_UNDEFINED),
            AmfValue::Null => self.buf.put_u8(MARKER_NULL),
            AmfValue::Boolean(false) => self.buf.put_u8(MARKER_FALSE),
            AmfValue::Boolean(true) => self.buf.put_u8(MARKER_TRUE),
            AmfValue::Integer(i) if *i >= AMF3_INT_MIN && *i <= AMF3_INT_MAX => {
                self.buf.put_u8(MARKER_INTEGER);
                self.write_u29(*i as u32 & 0x1FFFFFFF);
            }
            AmfValue::Integer(i) => {
                self.buf.put_u8(MARKER_DOUBLE);
                self.buf.put_f64(*i as f64);
            }
            AmfValue::Number(n) => {
                self.buf.put_u8(MARKER_DOUBLE);
                self.buf.put_f64(*n);
            }
            AmfValue::String(s) => {
                self.buf.put_u8(MARKER_STRING);
                self.write_string(s);
            }
            AmfValue::Array(elements) => {
                self.buf.put_u8(MARKER_ARRAY);
                let header = ((elements.len() as u32) << 1) | 1;
                self.write_u29(header);
                // Empty associative portion
                self.write_u29(1); // Empty string marker
                for elem in elements {
                    self.encode(elem);
                }
            }
            AmfValue::Object(props) | AmfValue::EcmaArray(props) => {
                self.buf.put_u8(MARKER_OBJECT);
                // Dynamic anonymous object
                let header = (0 << 4) | (1 << 3) | (1 << 2) | (1 << 1) | 1;
                self.write_u29(header);
                self.write_string(""); // Empty class name
                for (key, val) in props {
                    self.write_string(key);
                    self.encode(val);
                }
                self.write_string(""); // End marker
            }
            AmfValue::TypedObject { class_name, properties } => {
                self.buf.put_u8(MARKER_OBJECT);
                let header = (0 << 4) | (1 << 3) | (1 << 2) | (1 << 1) | 1;
                self.write_u29(header);
                self.write_string(class_name);
                for (key, val) in properties {
                    self.write_string(key);
                    self.encode(val);
                }
                self.write_string("");
            }
            AmfValue::Date(timestamp) => {
                self.buf.put_u8(MARKER_DATE);
                self.write_u29(1); // Inline
                self.buf.put_f64(*timestamp);
            }
            AmfValue::Xml(s) => {
                self.buf.put_u8(MARKER_XML);
                let header = ((s.len() as u32) << 1) | 1;
                self.write_u29(header);
                self.buf.put_slice(s.as_bytes());
            }
            AmfValue::ByteArray(data) => {
                self.buf.put_u8(MARKER_BYTE_ARRAY);
                let header = ((data.len() as u32) << 1) | 1;
                self.write_u29(header);
                self.buf.put_slice(data);
            }
        }
    }

    /// Write U29 variable-length integer
    fn write_u29(&mut self, value: u32) {
        let value = value & 0x1FFFFFFF;

        if value < 0x80 {
            self.buf.put_u8(value as u8);
        } else if value < 0x4000 {
            self.buf.put_u8(((value >> 7) | 0x80) as u8);
            self.buf.put_u8((value & 0x7F) as u8);
        } else if value < 0x200000 {
            self.buf.put_u8(((value >> 14) | 0x80) as u8);
            self.buf.put_u8(((value >> 7) | 0x80) as u8);
            self.buf.put_u8((value & 0x7F) as u8);
        } else {
            self.buf.put_u8(((value >> 22) | 0x80) as u8);
            self.buf.put_u8(((value >> 15) | 0x80) as u8);
            self.buf.put_u8(((value >> 8) | 0x80) as u8);
            self.buf.put_u8((value & 0xFF) as u8);
        }
    }

    /// Write string with reference handling
    fn write_string(&mut self, s: &str) {
        if s.is_empty() {
            self.write_u29(1); // Empty string marker
            return;
        }

        if let Some(&idx) = self.string_refs.get(s) {
            // Reference
            self.write_u29((idx as u32) << 1);
        } else {
            // Inline
            let idx = self.string_refs.len();
            self.string_refs.insert(s.to_string(), idx);
            let header = ((s.len() as u32) << 1) | 1;
            self.write_u29(header);
            self.buf.put_slice(s.as_bytes());
        }
    }
}

impl Default for Amf3Encoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u29_encoding() {
        let mut encoder = Amf3Encoder::new();

        // Test various ranges
        encoder.write_u29(0);
        encoder.write_u29(127);
        encoder.write_u29(128);
        encoder.write_u29(16383);
        encoder.write_u29(16384);
        encoder.write_u29(2097151);
        encoder.write_u29(2097152);

        let encoded = encoder.finish();

        let mut decoder = Amf3Decoder::new();
        let mut buf = encoded;

        assert_eq!(decoder.read_u29(&mut buf).unwrap(), 0);
        assert_eq!(decoder.read_u29(&mut buf).unwrap(), 127);
        assert_eq!(decoder.read_u29(&mut buf).unwrap(), 128);
        assert_eq!(decoder.read_u29(&mut buf).unwrap(), 16383);
        assert_eq!(decoder.read_u29(&mut buf).unwrap(), 16384);
        assert_eq!(decoder.read_u29(&mut buf).unwrap(), 2097151);
        assert_eq!(decoder.read_u29(&mut buf).unwrap(), 2097152);
    }

    #[test]
    fn test_string_roundtrip() {
        let mut encoder = Amf3Encoder::new();
        encoder.encode(&AmfValue::String("hello".into()));
        let encoded = encoder.finish();

        let mut decoder = Amf3Decoder::new();
        let mut buf = encoded;
        let decoded = decoder.decode(&mut buf).unwrap();
        assert_eq!(decoded, AmfValue::String("hello".into()));
    }
}
