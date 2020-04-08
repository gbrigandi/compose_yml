//! Tools for working with fields that might contain a string, or might
//! contain a struct.

use serde::de::{self, Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};
use std::fmt::{self, Display};
use std::marker::PhantomData;
use std::str::FromStr;

/// Handle a value which may either a struct that deserializes as type `T`,
/// or a bare string that can be turned into a type `T` using
/// `FromStr::from_str`.  We do this in a clever way that allows us to be
/// generic over multiple such types, and which allows us to use the
/// structure deserialization code automatically generated by serde.
///
/// An earlier and much uglier version of this parser was the inspiration
/// for [this official `serde`
/// example](https://serde.rs/string-or-struct.html), which in turn forms
/// the basis of this code.
pub fn deserialize_string_or_struct<'de, T, D>(d: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr,
    <T as FromStr>::Err: Display,
    D: Deserializer<'de>,
{
    /// Declare an internal visitor type to handle our input.
    struct StringOrStruct<T>(PhantomData<T>);

    impl<'de, T> de::Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr,
        <T as FromStr>::Err: Display,
    {
        type Value = T;

        fn visit_str<E>(self, value: &str) -> Result<T, E>
        where
            E: de::Error,
        {
            FromStr::from_str(value).map_err(|err| {
                // Just convert the underlying error type into a string and
                // pass it to serde as a custom error.
                de::Error::custom(format!("{}", err))
            })
        }

        fn visit_map<M>(self, visitor: M) -> Result<T, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let mvd = de::value::MapAccessDeserializer::new(visitor);
            Deserialize::deserialize(mvd)
        }

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a string or a map")
        }
    }

    d.deserialize_any(StringOrStruct(PhantomData))
}

/// Like `opt_string_or_struct`, but it also handles the case where the
/// value is optional.
///
/// We could probably make this more generic, supporting underlying
/// functions other than `string_or_struct`, but that's a project for
/// another day.
pub fn deserialize_opt_string_or_struct<'de, T, D>(d: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de> + FromStr,
    <T as FromStr>::Err: Display,
    D: Deserializer<'de>,
{
    /// Declare an internal visitor type to handle our input.
    struct OptStringOrStruct<T>(PhantomData<T>);

    impl<'de, T> de::Visitor<'de> for OptStringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr,
        <T as FromStr>::Err: Display,
    {
        type Value = Option<T>;

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserialize_string_or_struct(deserializer).map(Some)
        }

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a null, a string or a map")
        }
    }

    d.deserialize_option(OptStringOrStruct(PhantomData))
}

/// Some structs can serialized as a string, but only under certain
/// circumstances.
pub trait SerializeStringOrStruct: Serialize {
    /// Serialize either a string representation of this struct, or a full
    /// struct if the object cannot be represented as a string.
    fn serialize_string_or_struct<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
}

/// Serialize the specified value as a string if we can, and a struct
/// otherwise.
pub fn serialize_string_or_struct<T, S>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: SerializeStringOrStruct,
    S: Serializer,
{
    value.serialize_string_or_struct(serializer)
}

/// Like `serialize_string_or_struct`, but can also handle missing values.
pub fn serialize_opt_string_or_struct<T, S>(
    value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: SerializeStringOrStruct,
    S: Serializer,
{
    /// A fun little trick: We need to pass `value` to to `serialize_some`,
    /// but we don't want `serialize_some` to call the normal `serialize`
    /// method on it.  So we define a local wrapper type that overrides the
    /// serialization.  This is one of the more subtle tricks of generic
    /// programming in Rust: using a "newtype" wrapper struct to override
    /// how a trait is applied to a class.
    struct Wrap<'a, T>(&'a T)
    where
        T: SerializeStringOrStruct;

    impl<'a, T> Serialize for Wrap<'a, T>
    where
        T: 'a + SerializeStringOrStruct,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match *self {
                Wrap(v) => serialize_string_or_struct(v, serializer),
            }
        }
    }

    match *value {
        None => serializer.serialize_none(),
        Some(ref v) => serializer.serialize_some(&Wrap(v)),
    }
}
