//! Field-level reflection for settings structs. `#[derive(Settings)]` lists the fields
//! marked `#[setting(..)]` in a generated `<Struct>Field` enum and describes each one
//! with a [`Schema`]: its key, its label, and whether it is a toggle, a bounded number
//! or a choice from a list. Code that presents settings walks the schema and reads or
//! writes any field through [`Settings::get`] and [`Settings::set`] instead of carrying
//! glue for every field. Nothing here draws or depends on a UI toolkit.

pub use settings_schema_derive::Settings;

pub trait Settings {
    /// The generated `<Struct>Field` enum, with one variant per `#[setting]` field.
    type Field: Copy + Eq + 'static;

    /// Every setting field in declaration order.
    const FIELDS: &'static [Self::Field];

    fn schema(field: Self::Field) -> &'static Schema;

    fn get(&self, field: Self::Field) -> Value;

    /// Stores `value` once [`Kind::normalize`] accepts it and, for a choice, once it
    /// parses into the field type. Returns whether the field was written.
    fn set(&mut self, field: Self::Field, value: Value) -> bool;
}

#[derive(Debug)]
pub struct Schema {
    /// The field name.
    pub key: &'static str,
    /// The `label = ".."` argument, or the field name when there is none.
    pub label: &'static str,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    Toggle,
    Number(Number),
    Choice(&'static [Choice]),
}

impl Kind {
    /// Returns the value a field of this kind would store: numbers are rounded to their
    /// precision and clamped, while a value of another kind or an unlisted choice is
    /// rejected.
    pub fn normalize(&self, value: Value) -> Option<Value> {
        match (self, value) {
            (Self::Toggle, Value::Bool(on)) => Some(Value::Bool(on)),
            (Self::Number(number), Value::Number(value)) if value.is_finite() => {
                Some(Value::Number(number.normalize(value)))
            }
            (Self::Choice(choices), Value::Text(text))
                if choices.iter().any(|choice| choice.value == text) =>
            {
                Some(Value::Text(text))
            }
            _ => None,
        }
    }
}

/// Limits of a numeric setting. Omitted bounds are infinite, and `precision` is the
/// number of decimals that are kept and shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Number {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub precision: u8,
}

impl Number {
    pub fn normalize(&self, value: f64) -> f64 {
        let scale = 10f64.powi(i32::from(self.precision));
        ((value * scale).round() / scale).clamp(self.min, self.max)
    }

    /// Moves `value` by `steps` steps, negative to go down, and normalizes the result.
    pub fn stepped(&self, value: f64, steps: i32) -> f64 {
        self.normalize(value + self.step * f64::from(steps))
    }

    pub fn format(&self, value: f64) -> String {
        format!("{value:.*}", usize::from(self.precision))
    }
}

#[derive(Debug)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Number(f64),
    Text(String),
}
