use serde_json::{Map, Value};
use std::borrow::Borrow;

/// Extension trait for easier JSON value extraction.
pub trait JsonExt {
    fn get_str(&self, path: &str) -> Option<&str>;
    fn get_u64(&self, path: &str) -> Option<u64>;
    fn get_i64(&self, path: &str) -> Option<i64>;
    fn get_f64(&self, path: &str) -> Option<f64>;
    fn get_bool(&self, path: &str) -> Option<bool>;
    fn get_array(&self, path: &str) -> Option<&Vec<Value>>;
    fn get_object(&self, path: &str) -> Option<&Map<String, Value>>;
    fn get_value(&self, path: &str) -> Option<&Value>;
}

// Implementation for direct Value
impl JsonExt for Value {
    fn get_str(&self, path: &str) -> Option<&str> {
        self.pointer(&format!("/{}", path))?.as_str()
    }

    fn get_u64(&self, path: &str) -> Option<u64> {
        self.pointer(&format!("/{}", path))?.as_u64()
    }

    fn get_i64(&self, path: &str) -> Option<i64> {
        self.pointer(&format!("/{}", path))?.as_i64()
    }

    fn get_f64(&self, path: &str) -> Option<f64> {
        self.pointer(&format!("/{}", path))?.as_f64()
    }

    fn get_bool(&self, path: &str) -> Option<bool> {
        self.pointer(&format!("/{}", path))?.as_bool()
    }

    fn get_array(&self, path: &str) -> Option<&Vec<Value>> {
        self.pointer(&format!("/{}", path))?.as_array()
    }

    fn get_object(&self, path: &str) -> Option<&Map<String, Value>> {
        self.pointer(&format!("/{}", path))?.as_object()
    }

    fn get_value(&self, path: &str) -> Option<&Value> {
        self.pointer(&format!("/{}", path))
    }
}

impl<T: Borrow<Value>> JsonExt for Option<T> {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.as_ref()?.borrow().get_str(key)
    }

    fn get_u64(&self, key: &str) -> Option<u64> {
        self.as_ref()?.borrow().get_u64(key)
    }

    fn get_i64(&self, key: &str) -> Option<i64> {
        self.as_ref()?.borrow().get_i64(key)
    }

    fn get_f64(&self, key: &str) -> Option<f64> {
        self.as_ref()?.borrow().get_f64(key)
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.as_ref()?.borrow().get_bool(key)
    }

    fn get_array(&self, key: &str) -> Option<&Vec<Value>> {
        self.as_ref()?.borrow().get_array(key)
    }

    fn get_object(&self, key: &str) -> Option<&Map<String, Value>> {
        self.as_ref()?.borrow().get_object(key)
    }

    fn get_value(&self, key: &str) -> Option<&Value> {
        self.as_ref()?.borrow().get_value(key)
    }
}
