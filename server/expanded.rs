#![feature(prelude_import)]
#![feature(assert_matches)]
#![allow(clippy::style)]
#[prelude_import]
use std::prelude::rust_2018::*;
#[macro_use]
extern crate std;
pub mod config {
    //! Describes a server config which may be parsed from a TOML file.
    use std::collections::{btree_map, BTreeMap};
    use std::num::NonZeroUsize;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use std::sync::Arc;
    use serde_derive::{Deserialize, Serialize};
    use thiserror::Error;
    use propolis::block;
    use propolis::dispatch::Dispatcher;
    use propolis::inventory;
    /// Errors which may be returned when parsing the server configuration.
    pub enum ParseError {
        #[error("Cannot parse toml: {0}")]
        Toml(#[from] toml::de::Error),
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),
        #[error("Key {0} not found in {1}")]
        KeyNotFound(String, String),
        #[error("Could not unmarshall {0} with function {1}")]
        AsError(String, String),
    }
    #[allow(unused_qualifications)]
    impl std::error::Error for ParseError {
        fn source(&self) -> std::option::Option<&(dyn std::error::Error + 'static)> {
            use thiserror::private::AsDynError;
            #[allow(deprecated)]
            match self {
                ParseError::Toml { 0: source, .. } => {
                    std::option::Option::Some(source.as_dyn_error())
                }
                ParseError::Io { 0: source, .. } => {
                    std::option::Option::Some(source.as_dyn_error())
                }
                ParseError::KeyNotFound { .. } => std::option::Option::None,
                ParseError::AsError { .. } => std::option::Option::None,
            }
        }
    }
    #[allow(unused_qualifications)]
    impl std::fmt::Display for ParseError {
        fn fmt(&self, __formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            #[allow(unused_imports)]
            use thiserror::private::{DisplayAsDisplay, PathAsDisplay};
            #[allow(unused_variables, deprecated, clippy::used_underscore_binding)]
            match self {
                ParseError::Toml(_0) => __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                    &["Cannot parse toml: "],
                    &match (&_0.as_display(),) {
                        _args => [::core::fmt::ArgumentV1::new(
                            _args.0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                )),
                ParseError::Io(_0) => __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                    &["IO error: "],
                    &match (&_0.as_display(),) {
                        _args => [::core::fmt::ArgumentV1::new(
                            _args.0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                )),
                ParseError::KeyNotFound(_0, _1) => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["Key ", " not found in "],
                        &match (&_0.as_display(), &_1.as_display()) {
                            _args => [
                                ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                                ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                            ],
                        },
                    ))
                }
                ParseError::AsError(_0, _1) => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["Could not unmarshall ", " with function "],
                        &match (&_0.as_display(), &_1.as_display()) {
                            _args => [
                                ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                                ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                            ],
                        },
                    ))
                }
            }
        }
    }
    #[allow(unused_qualifications)]
    impl std::convert::From<toml::de::Error> for ParseError {
        #[allow(deprecated)]
        fn from(source: toml::de::Error) -> Self {
            ParseError::Toml { 0: source }
        }
    }
    #[allow(unused_qualifications)]
    impl std::convert::From<std::io::Error> for ParseError {
        #[allow(deprecated)]
        fn from(source: std::io::Error) -> Self {
            ParseError::Io { 0: source }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for ParseError {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match (&*self,) {
                (&ParseError::Toml(ref __self_0),) => {
                    let debug_trait_builder = &mut ::core::fmt::Formatter::debug_tuple(f, "Toml");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&ParseError::Io(ref __self_0),) => {
                    let debug_trait_builder = &mut ::core::fmt::Formatter::debug_tuple(f, "Io");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&ParseError::KeyNotFound(ref __self_0, ref __self_1),) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "KeyNotFound");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&ParseError::AsError(ref __self_0, ref __self_1),) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "AsError");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
            }
        }
    }
    /// Configuration for the Propolis server.
    pub struct Config {
        bootrom: PathBuf,
        #[serde(default, rename = "dev")]
        devices: BTreeMap<String, Device>,
        #[serde(default, rename = "block_dev")]
        block_devs: BTreeMap<String, BlockDevice>,
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for Config {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                let mut __serde_state = match _serde::Serializer::serialize_struct(
                    __serializer,
                    "Config",
                    false as usize + 1 + 1 + 1,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "bootrom",
                    &self.bootrom,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "dev",
                    &self.devices,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "block_dev",
                    &self.block_devs,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for Config {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field {
                    __field0,
                    __field1,
                    __field2,
                    __ignore,
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "field identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            2u64 => _serde::__private::Ok(__Field::__field2),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "bootrom" => _serde::__private::Ok(__Field::__field0),
                            "dev" => _serde::__private::Ok(__Field::__field1),
                            "block_dev" => _serde::__private::Ok(__Field::__field2),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"bootrom" => _serde::__private::Ok(__Field::__field0),
                            b"dev" => _serde::__private::Ok(__Field::__field1),
                            b"block_dev" => _serde::__private::Ok(__Field::__field2),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<Config>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = Config;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "struct Config")
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 = match match _serde::de::SeqAccess::next_element::<PathBuf>(
                            &mut __seq,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(_serde::de::Error::invalid_length(
                                    0usize,
                                    &"struct Config with 3 elements",
                                ));
                            }
                        };
                        let __field1 = match match _serde::de::SeqAccess::next_element::<
                            BTreeMap<String, Device>,
                        >(&mut __seq)
                        {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => _serde::__private::Default::default(),
                        };
                        let __field2 = match match _serde::de::SeqAccess::next_element::<
                            BTreeMap<String, BlockDevice>,
                        >(&mut __seq)
                        {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => _serde::__private::Default::default(),
                        };
                        _serde::__private::Ok(Config {
                            bootrom: __field0,
                            devices: __field1,
                            block_devs: __field2,
                        })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<PathBuf> =
                            _serde::__private::None;
                        let mut __field1: _serde::__private::Option<BTreeMap<String, Device>> =
                            _serde::__private::None;
                        let mut __field2: _serde::__private::Option<BTreeMap<String, BlockDevice>> =
                            _serde::__private::None;
                        while let _serde::__private::Some(__key) =
                            match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            }
                        {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "bootrom",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<PathBuf>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field1 => {
                                    if _serde::__private::Option::is_some(&__field1) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "dev",
                                            ),
                                        );
                                    }
                                    __field1 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<
                                            BTreeMap<String, Device>,
                                        >(&mut __map)
                                        {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field2 => {
                                    if _serde::__private::Option::is_some(&__field2) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "block_dev",
                                            ),
                                        );
                                    }
                                    __field2 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<
                                            BTreeMap<String, BlockDevice>,
                                        >(&mut __map)
                                        {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                _ => {
                                    let _ = match _serde::de::MapAccess::next_value::<
                                        _serde::de::IgnoredAny,
                                    >(&mut __map)
                                    {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    };
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("bootrom") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field1 = match __field1 {
                            _serde::__private::Some(__field1) => __field1,
                            _serde::__private::None => _serde::__private::Default::default(),
                        };
                        let __field2 = match __field2 {
                            _serde::__private::Some(__field2) => __field2,
                            _serde::__private::None => _serde::__private::Default::default(),
                        };
                        _serde::__private::Ok(Config {
                            bootrom: __field0,
                            devices: __field1,
                            block_devs: __field2,
                        })
                    }
                }
                const FIELDS: &'static [&'static str] = &["bootrom", "dev", "block_dev"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "Config",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<Config>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for Config {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                Config {
                    bootrom: ref __self_0_0,
                    devices: ref __self_0_1,
                    block_devs: ref __self_0_2,
                } => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_struct(f, "Config");
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "bootrom",
                        &&(*__self_0_0),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "devices",
                        &&(*__self_0_1),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "block_devs",
                        &&(*__self_0_2),
                    );
                    ::core::fmt::DebugStruct::finish(debug_trait_builder)
                }
            }
        }
    }
    impl Config {
        /// Constructs a new configuration object.
        ///
        /// Typically, the configuration is parsed from a config
        /// file via [`parse`], but this method allows an alternative
        /// mechanism for initialization.
        pub fn new<P: Into<PathBuf>>(
            bootrom: P,
            devices: BTreeMap<String, Device>,
            block_devs: BTreeMap<String, BlockDevice>,
        ) -> Config {
            Config {
                bootrom: bootrom.into(),
                devices,
                block_devs,
            }
        }
        pub fn get_bootrom(&self) -> &Path {
            &self.bootrom
        }
        pub fn devs(&self) -> IterDevs {
            IterDevs {
                inner: self.devices.iter(),
            }
        }
        pub fn create_block_backend(
            &self,
            name: &str,
            disp: &Dispatcher,
        ) -> Result<(Arc<dyn block::Backend>, inventory::ChildRegister), ParseError> {
            let entry = self.block_devs.get(name).ok_or_else(|| {
                ParseError::KeyNotFound(name.to_string(), "block_dev".to_string())
            })?;
            entry.create_block_backend(disp)
        }
    }
    /// A hard-coded device, either enabled by default or accessible locally
    /// on a machine.
    pub struct Device {
        pub driver: String,
        #[serde(flatten, default)]
        pub options: BTreeMap<String, toml::Value>,
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for Device {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                let mut __serde_state = match _serde::Serializer::serialize_map(
                    __serializer,
                    _serde::__private::None,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeMap::serialize_entry(
                    &mut __serde_state,
                    "driver",
                    &self.driver,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::Serialize::serialize(
                    &&self.options,
                    _serde::__private::ser::FlatMapSerializer(&mut __serde_state),
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                _serde::ser::SerializeMap::end(__serde_state)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for Device {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field<'de> {
                    __field0,
                    __other(_serde::__private::de::Content<'de>),
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field<'de>;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "field identifier")
                    }
                    fn visit_bool<__E>(
                        self,
                        __value: bool,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::Bool(__value),
                        ))
                    }
                    fn visit_i8<__E>(
                        self,
                        __value: i8,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(_serde::__private::de::Content::I8(
                            __value,
                        )))
                    }
                    fn visit_i16<__E>(
                        self,
                        __value: i16,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::I16(__value),
                        ))
                    }
                    fn visit_i32<__E>(
                        self,
                        __value: i32,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::I32(__value),
                        ))
                    }
                    fn visit_i64<__E>(
                        self,
                        __value: i64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::I64(__value),
                        ))
                    }
                    fn visit_u8<__E>(
                        self,
                        __value: u8,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(_serde::__private::de::Content::U8(
                            __value,
                        )))
                    }
                    fn visit_u16<__E>(
                        self,
                        __value: u16,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::U16(__value),
                        ))
                    }
                    fn visit_u32<__E>(
                        self,
                        __value: u32,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::U32(__value),
                        ))
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::U64(__value),
                        ))
                    }
                    fn visit_f32<__E>(
                        self,
                        __value: f32,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::F32(__value),
                        ))
                    }
                    fn visit_f64<__E>(
                        self,
                        __value: f64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::F64(__value),
                        ))
                    }
                    fn visit_char<__E>(
                        self,
                        __value: char,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::Char(__value),
                        ))
                    }
                    fn visit_unit<__E>(self) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::Unit,
                        ))
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "driver" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value = _serde::__private::de::Content::String(
                                    _serde::__private::ToString::to_string(__value),
                                );
                                _serde::__private::Ok(__Field::__other(__value))
                            }
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"driver" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value =
                                    _serde::__private::de::Content::ByteBuf(__value.to_vec());
                                _serde::__private::Ok(__Field::__other(__value))
                            }
                        }
                    }
                    fn visit_borrowed_str<__E>(
                        self,
                        __value: &'de str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "driver" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value = _serde::__private::de::Content::Str(__value);
                                _serde::__private::Ok(__Field::__other(__value))
                            }
                        }
                    }
                    fn visit_borrowed_bytes<__E>(
                        self,
                        __value: &'de [u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"driver" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value = _serde::__private::de::Content::Bytes(__value);
                                _serde::__private::Ok(__Field::__other(__value))
                            }
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field<'de> {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<Device>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = Device;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "struct Device")
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<String> =
                            _serde::__private::None;
                        let mut __collect = _serde::__private::Vec::<
                            _serde::__private::Option<(
                                _serde::__private::de::Content,
                                _serde::__private::de::Content,
                            )>,
                        >::new();
                        while let _serde::__private::Some(__key) =
                            match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            }
                        {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "driver",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<String>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__other(__name) => {
                                    __collect.push(_serde::__private::Some((
                                        __name,
                                        match _serde::de::MapAccess::next_value(&mut __map) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    )));
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("driver") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field1: BTreeMap<String, toml::Value> =
                            match _serde::de::Deserialize::deserialize(
                                _serde::__private::de::FlatMapDeserializer(
                                    &mut __collect,
                                    _serde::__private::PhantomData,
                                ),
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            };
                        _serde::__private::Ok(Device {
                            driver: __field0,
                            options: __field1,
                        })
                    }
                }
                _serde::Deserializer::deserialize_map(
                    __deserializer,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<Device>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for Device {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                Device {
                    driver: ref __self_0_0,
                    options: ref __self_0_1,
                } => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_struct(f, "Device");
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "driver",
                        &&(*__self_0_0),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "options",
                        &&(*__self_0_1),
                    );
                    ::core::fmt::DebugStruct::finish(debug_trait_builder)
                }
            }
        }
    }
    impl Device {
        pub fn get_string<S: AsRef<str>>(&self, key: S) -> Option<&str> {
            self.options.get(key.as_ref())?.as_str()
        }
        pub fn get<T: FromStr, S: AsRef<str>>(&self, key: S) -> Option<T> {
            self.get_string(key)?.parse().ok()
        }
    }
    pub struct BlockDevice {
        #[serde(default, rename = "type")]
        pub bdtype: String,
        #[serde(flatten, default)]
        pub options: BTreeMap<String, toml::Value>,
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for BlockDevice {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                let mut __serde_state = match _serde::Serializer::serialize_map(
                    __serializer,
                    _serde::__private::None,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeMap::serialize_entry(
                    &mut __serde_state,
                    "type",
                    &self.bdtype,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::Serialize::serialize(
                    &&self.options,
                    _serde::__private::ser::FlatMapSerializer(&mut __serde_state),
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                _serde::ser::SerializeMap::end(__serde_state)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for BlockDevice {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field<'de> {
                    __field0,
                    __other(_serde::__private::de::Content<'de>),
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field<'de>;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "field identifier")
                    }
                    fn visit_bool<__E>(
                        self,
                        __value: bool,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::Bool(__value),
                        ))
                    }
                    fn visit_i8<__E>(
                        self,
                        __value: i8,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(_serde::__private::de::Content::I8(
                            __value,
                        )))
                    }
                    fn visit_i16<__E>(
                        self,
                        __value: i16,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::I16(__value),
                        ))
                    }
                    fn visit_i32<__E>(
                        self,
                        __value: i32,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::I32(__value),
                        ))
                    }
                    fn visit_i64<__E>(
                        self,
                        __value: i64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::I64(__value),
                        ))
                    }
                    fn visit_u8<__E>(
                        self,
                        __value: u8,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(_serde::__private::de::Content::U8(
                            __value,
                        )))
                    }
                    fn visit_u16<__E>(
                        self,
                        __value: u16,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::U16(__value),
                        ))
                    }
                    fn visit_u32<__E>(
                        self,
                        __value: u32,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::U32(__value),
                        ))
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::U64(__value),
                        ))
                    }
                    fn visit_f32<__E>(
                        self,
                        __value: f32,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::F32(__value),
                        ))
                    }
                    fn visit_f64<__E>(
                        self,
                        __value: f64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::F64(__value),
                        ))
                    }
                    fn visit_char<__E>(
                        self,
                        __value: char,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::Char(__value),
                        ))
                    }
                    fn visit_unit<__E>(self) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        _serde::__private::Ok(__Field::__other(
                            _serde::__private::de::Content::Unit,
                        ))
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "type" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value = _serde::__private::de::Content::String(
                                    _serde::__private::ToString::to_string(__value),
                                );
                                _serde::__private::Ok(__Field::__other(__value))
                            }
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"type" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value =
                                    _serde::__private::de::Content::ByteBuf(__value.to_vec());
                                _serde::__private::Ok(__Field::__other(__value))
                            }
                        }
                    }
                    fn visit_borrowed_str<__E>(
                        self,
                        __value: &'de str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "type" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value = _serde::__private::de::Content::Str(__value);
                                _serde::__private::Ok(__Field::__other(__value))
                            }
                        }
                    }
                    fn visit_borrowed_bytes<__E>(
                        self,
                        __value: &'de [u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"type" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value = _serde::__private::de::Content::Bytes(__value);
                                _serde::__private::Ok(__Field::__other(__value))
                            }
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field<'de> {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<BlockDevice>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = BlockDevice;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "struct BlockDevice")
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<String> =
                            _serde::__private::None;
                        let mut __collect = _serde::__private::Vec::<
                            _serde::__private::Option<(
                                _serde::__private::de::Content,
                                _serde::__private::de::Content,
                            )>,
                        >::new();
                        while let _serde::__private::Some(__key) =
                            match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            }
                        {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "type",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<String>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__other(__name) => {
                                    __collect.push(_serde::__private::Some((
                                        __name,
                                        match _serde::de::MapAccess::next_value(&mut __map) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    )));
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => _serde::__private::Default::default(),
                        };
                        let __field1: BTreeMap<String, toml::Value> =
                            match _serde::de::Deserialize::deserialize(
                                _serde::__private::de::FlatMapDeserializer(
                                    &mut __collect,
                                    _serde::__private::PhantomData,
                                ),
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            };
                        _serde::__private::Ok(BlockDevice {
                            bdtype: __field0,
                            options: __field1,
                        })
                    }
                }
                _serde::Deserializer::deserialize_map(
                    __deserializer,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<BlockDevice>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for BlockDevice {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                BlockDevice {
                    bdtype: ref __self_0_0,
                    options: ref __self_0_1,
                } => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_struct(f, "BlockDevice");
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "bdtype",
                        &&(*__self_0_0),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "options",
                        &&(*__self_0_1),
                    );
                    ::core::fmt::DebugStruct::finish(debug_trait_builder)
                }
            }
        }
    }
    impl BlockDevice {
        pub fn create_block_backend(
            &self,
            _disp: &Dispatcher,
        ) -> Result<(Arc<dyn block::Backend>, inventory::ChildRegister), ParseError> {
            match &self.bdtype as &str {
                "file" => {
                    let path = self
                        .options
                        .get("path")
                        .ok_or_else(|| {
                            ParseError::KeyNotFound("path".to_string(), "options".to_string())
                        })?
                        .as_str()
                        .ok_or_else(|| {
                            ParseError::AsError("path".to_string(), "as_str".to_string())
                        })?;
                    let readonly: bool = (|| -> Option<bool> {
                        self.options.get("readonly")?.as_str()?.parse().ok()
                    })()
                    .unwrap_or(false);
                    let nworkers = NonZeroUsize::new(8).unwrap();
                    let be = propolis::block::FileBackend::create(path, readonly, nworkers)?;
                    let child = inventory::ChildRegister::new(&be, None);
                    Ok((be, child))
                }
                _ => {
                    {
                        ::std::rt::panic_fmt(::core::fmt::Arguments::new_v1(
                            &["unrecognized block dev type ", "!"],
                            &match (&self.bdtype,) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ))
                    };
                }
            }
        }
    }
    /// Iterator returned from [`Config::devs`] which allows iteration over
    /// all [`Device`] objects.
    pub struct IterDevs<'a> {
        inner: btree_map::Iter<'a, String, Device>,
    }
    impl<'a> Iterator for IterDevs<'a> {
        type Item = (&'a String, &'a Device);
        fn next(&mut self) -> Option<Self::Item> {
            self.inner.next()
        }
    }
    /// Parses a TOML file into a configuration object.
    pub fn parse<P: AsRef<Path>>(path: P) -> Result<Config, ParseError> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        let cfg = toml::from_str::<Config>(&contents)?;
        Ok(cfg)
    }
}
mod initializer {
    use std::fs::File;
    use std::io::{Error, ErrorKind};
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::time::SystemTime;
    use propolis::block;
    use propolis::chardev::{self, BlockingSource, Source};
    use propolis::common::PAGE_SIZE;
    use propolis::dispatch::Dispatcher;
    use propolis::hw::chipset::{i440fx::I440Fx, Chipset};
    use propolis::hw::ibmpc;
    use propolis::hw::pci;
    use propolis::hw::ps2ctrl::PS2Ctrl;
    use propolis::hw::qemu::{debug::QemuDebugPort, fwcfg, ramfb};
    use propolis::hw::uart::LpcUart;
    use propolis::hw::{nvme, virtio};
    use propolis::instance::Instance;
    use propolis::inventory::{ChildRegister, EntityID, Inventory};
    use propolis::vmm::{self, Builder, Machine, MachineCtx, Prot};
    use slog::info;
    use crate::serial::Serial;
    use anyhow::Result;
    use tokio::runtime::Handle;
    const MAX_ROM_SIZE: usize = 0x20_0000;
    fn open_bootrom<P: AsRef<std::path::Path>>(path: P) -> Result<(File, usize)> {
        let fp = File::open(path.as_ref())?;
        let len = fp.metadata()?.len();
        if len % (PAGE_SIZE as u64) != 0 {
            Err(Error::new(ErrorKind::InvalidData, {
                let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                    &["rom ", " length ", " not aligned to "],
                    &match (&path.as_ref().to_string_lossy(), &len, &PAGE_SIZE) {
                        _args => [
                            ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                            ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::LowerHex::fmt),
                            ::core::fmt::ArgumentV1::new(_args.2, ::core::fmt::LowerHex::fmt),
                        ],
                    },
                ));
                res
            })
            .into())
        } else {
            Ok((fp, len as usize))
        }
    }
    pub fn build_instance(
        name: &str,
        max_cpu: u8,
        lowmem: usize,
        highmem: usize,
        log: slog::Logger,
    ) -> Result<Arc<Instance>> {
        let mut builder = Builder::new(name, true)?
            .max_cpus(max_cpu)?
            .add_mem_region(0, lowmem, Prot::ALL, "lowmem")?
            .add_rom_region(
                0x1_0000_0000 - MAX_ROM_SIZE,
                MAX_ROM_SIZE,
                Prot::READ | Prot::EXEC,
                "bootrom",
            )?
            .add_mmio_region(0xc000_0000_usize, 0x2000_0000_usize, "dev32")?
            .add_mmio_region(0xe000_0000_usize, 0x1000_0000_usize, "pcicfg")?
            .add_mmio_region(vmm::MAX_SYSMEM, vmm::MAX_PHYSMEM - vmm::MAX_SYSMEM, "dev64")?;
        if highmem > 0 {
            builder = builder.add_mem_region(0x1_0000_0000, highmem, Prot::ALL, "highmem")?;
        }
        let rt_handle = Some(Handle::current());
        let inst = Instance::create(builder.finalize()?, rt_handle, Some(log))?;
        inst.spawn_vcpu_workers(propolis::vcpu_run_loop)?;
        Ok(inst)
    }
    pub struct RegisteredChipset(Arc<I440Fx>, EntityID);
    impl RegisteredChipset {
        pub fn device(&self) -> &Arc<I440Fx> {
            &self.0
        }
    }
    pub struct MachineInitializer<'a> {
        log: slog::Logger,
        machine: &'a Machine,
        mctx: &'a MachineCtx,
        disp: &'a Dispatcher,
        inv: &'a Inventory,
    }
    impl<'a> MachineInitializer<'a> {
        pub fn new(
            log: slog::Logger,
            machine: &'a Machine,
            mctx: &'a MachineCtx,
            disp: &'a Dispatcher,
            inv: &'a Inventory,
        ) -> Self {
            MachineInitializer {
                log,
                machine,
                mctx,
                disp,
                inv,
            }
        }
        pub fn initialize_rom<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Error> {
            let (romfp, rom_len) = open_bootrom(path.as_ref()).unwrap_or_else(|e| {
                ::std::rt::panic_fmt(::core::fmt::Arguments::new_v1(
                    &["Cannot open bootrom: "],
                    &match (&e,) {
                        _args => [::core::fmt::ArgumentV1::new(
                            _args.0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                ))
            });
            self.machine.populate_rom("bootrom", |mapping| {
                let mapping = mapping.as_ref();
                if mapping.len() < rom_len {
                    return Err(Error::new(ErrorKind::InvalidData, "rom too long"));
                }
                let offset = mapping.len() - rom_len;
                let submapping = mapping.subregion(offset, rom_len).unwrap();
                let nread = submapping.pread(&romfp, rom_len, 0)?;
                if nread != rom_len {
                    return Err(Error::new(ErrorKind::InvalidData, "short read"));
                }
                Ok(())
            })?;
            Ok(())
        }
        pub fn initialize_kernel_devs(&self, lowmem: usize, highmem: usize) -> Result<(), Error> {
            let hdl = self.mctx.hdl();
            let (pic, pit, hpet, ioapic, rtc) = propolis::hw::bhyve::defaults();
            rtc.memsize_to_nvram(lowmem, highmem, hdl)?;
            rtc.set_time(SystemTime::now(), hdl)?;
            self.inv.register(&pic)?;
            self.inv.register(&pit)?;
            self.inv.register(&hpet)?;
            self.inv.register(&ioapic)?;
            self.inv.register(&rtc)?;
            Ok(())
        }
        pub fn initialize_chipset(&self) -> Result<RegisteredChipset, Error> {
            let chipset = I440Fx::create(self.machine);
            let id = self.inv.register(&chipset)?;
            Ok(RegisteredChipset(chipset, id))
        }
        pub fn initialize_uart(
            &self,
            chipset: &RegisteredChipset,
        ) -> Result<Serial<LpcUart>, Error> {
            let uarts = <[_]>::into_vec(box [
                (ibmpc::IRQ_COM1, ibmpc::PORT_COM1, "com1"),
                (ibmpc::IRQ_COM2, ibmpc::PORT_COM2, "com2"),
                (ibmpc::IRQ_COM3, ibmpc::PORT_COM3, "com3"),
                (ibmpc::IRQ_COM4, ibmpc::PORT_COM4, "com4"),
            ]);
            let pio = self.mctx.pio();
            let mut com1 = None;
            for (irq, port, name) in uarts.iter() {
                let dev = LpcUart::new(chipset.device().irq_pin(*irq).unwrap());
                dev.set_autodiscard(true);
                LpcUart::attach(&dev, pio, *port);
                self.inv.register_instance(&dev, name)?;
                if com1.is_none() {
                    com1 = Some(dev);
                }
            }
            let sink_size = NonZeroUsize::new(64).unwrap();
            let source_size = NonZeroUsize::new(1024).unwrap();
            Ok(Serial::new(com1.unwrap(), sink_size, source_size))
        }
        pub fn initialize_ps2(&self, chipset: &RegisteredChipset) -> Result<(), Error> {
            let pio = self.mctx.pio();
            let ps2_ctrl = PS2Ctrl::create();
            ps2_ctrl.attach(pio, chipset.device().as_ref());
            self.inv.register(&ps2_ctrl)?;
            Ok(())
        }
        pub fn initialize_qemu_debug_port(&self) -> Result<(), Error> {
            let dbg = QemuDebugPort::create(self.mctx.pio());
            let debug_file = std::fs::File::create("debug.out")?;
            let poller = chardev::BlockingFileOutput::new(debug_file)?;
            poller.attach(Arc::clone(&dbg) as Arc<dyn BlockingSource>, self.disp);
            self.inv.register(&dbg)?;
            Ok(())
        }
        pub fn initialize_virtio_block(
            &self,
            chipset: &RegisteredChipset,
            bdf: pci::Bdf,
            backend: Arc<dyn block::Backend>,
            be_register: ChildRegister,
        ) -> Result<(), Error> {
            let be_info = backend.info();
            let vioblk = virtio::PciVirtioBlock::new(0x100, be_info);
            let id = self.inv.register_instance(&vioblk, bdf.to_string())?;
            let _ = self.inv.register_child(be_register, id).unwrap();
            backend.attach(vioblk.clone(), self.disp);
            chipset.device().pci_attach(bdf, vioblk);
            Ok(())
        }
        pub fn initialize_nvme_block(
            &self,
            chipset: &RegisteredChipset,
            bdf: pci::Bdf,
            name: String,
            backend: Arc<dyn block::Backend>,
            be_register: ChildRegister,
        ) -> Result<(), Error> {
            let be_info = backend.info();
            let nvme = nvme::PciNvme::create(0x1de, 0x1000, name, be_info);
            let id = self.inv.register_instance(&nvme, bdf.to_string())?;
            let _ = self.inv.register_child(be_register, id).unwrap();
            backend.attach(nvme.clone(), self.disp);
            chipset.device().pci_attach(bdf, nvme);
            Ok(())
        }
        pub fn initialize_vnic(
            &self,
            chipset: &RegisteredChipset,
            vnic_name: &str,
            bdf: pci::Bdf,
        ) -> Result<(), Error> {
            let hdl = self.machine.get_hdl();
            let viona = virtio::PciVirtioViona::new(vnic_name, 0x100, &hdl)?;
            let _id = self.inv.register_instance(&viona, bdf.to_string())?;
            chipset.device().pci_attach(bdf, viona);
            Ok(())
        }
        pub fn initialize_crucible(
            &self,
            chipset: &RegisteredChipset,
            disk: &propolis_client::api::DiskRequest,
            bdf: pci::Bdf,
        ) -> Result<(), Error> {
            if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                ::slog::Logger::log(&self.log, &{
                    #[allow(dead_code)]
                    static RS: ::slog::RecordStatic<'static> = {
                        static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                            file: "server/src/lib/initializer.rs",
                            line: 268u32,
                            column: 9u32,
                            function: "",
                            module: "propolis_server::initializer",
                        };
                        ::slog::RecordStatic {
                            location: &LOC,
                            level: ::slog::Level::Info,
                            tag: "",
                        }
                    };
                    ::slog::Record::new(
                        &RS,
                        &::core::fmt::Arguments::new_v1_formatted(
                            &["Creating Crucible disk from "],
                            &match (&disk,) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Debug::fmt,
                                )],
                            },
                            &[::core::fmt::rt::v1::Argument {
                                position: 0usize,
                                format: ::core::fmt::rt::v1::FormatSpec {
                                    fill: ' ',
                                    align: ::core::fmt::rt::v1::Alignment::Unknown,
                                    flags: 4u32,
                                    precision: ::core::fmt::rt::v1::Count::Implied,
                                    width: ::core::fmt::rt::v1::Count::Implied,
                                },
                            }],
                            unsafe { ::core::fmt::UnsafeArg::new() },
                        ),
                        ::slog::BorrowedKV(&()),
                    )
                })
            };
            let be = propolis::block::CrucibleBackend::create(
                self.disp,
                disk.gen,
                disk.volume_construction_request.clone(),
                disk.read_only,
            )?;
            if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                ::slog::Logger::log(&self.log, &{
                    #[allow(dead_code)]
                    static RS: ::slog::RecordStatic<'static> = {
                        static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                            file: "server/src/lib/initializer.rs",
                            line: 276u32,
                            column: 9u32,
                            function: "",
                            module: "propolis_server::initializer",
                        };
                        ::slog::RecordStatic {
                            location: &LOC,
                            level: ::slog::Level::Info,
                            tag: "",
                        }
                    };
                    ::slog::Record::new(
                        &RS,
                        &::core::fmt::Arguments::new_v1(
                            &["Creating ChildRegister"],
                            &match () {
                                _args => [],
                            },
                        ),
                        ::slog::BorrowedKV(&()),
                    )
                })
            };
            let creg = ChildRegister::new(&be, None);
            match disk.device.as_ref() {
                "virtio" => {
                    if ::slog::Level::Info.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log, &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/initializer.rs",
                                    line: 281u32,
                                    column: 17u32,
                                    function: "",
                                    module: "propolis_server::initializer",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Info,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["Calling initialize_virtio_block"],
                                    &match () {
                                        _args => [],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    self.initialize_virtio_block(chipset, bdf, be, creg)
                }
                "nvme" => {
                    if ::slog::Level::Info.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log, &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/initializer.rs",
                                    line: 285u32,
                                    column: 17u32,
                                    function: "",
                                    module: "propolis_server::initializer",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Info,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["Calling initialize_nvme_block"],
                                    &match () {
                                        _args => [],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    self.initialize_nvme_block(chipset, bdf, disk.name.clone(), be, creg)
                }
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Bad disk device!",
                )),
            }
        }
        pub fn initialize_in_memory_virtio_from_bytes(
            &self,
            chipset: &RegisteredChipset,
            bytes: Vec<u8>,
            bdf: pci::Bdf,
            read_only: bool,
        ) -> Result<(), Error> {
            if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                ::slog::Logger::log(&self.log, &{
                    #[allow(dead_code)]
                    static RS: ::slog::RecordStatic<'static> = {
                        static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                            file: "server/src/lib/initializer.rs",
                            line: 308u32,
                            column: 9u32,
                            function: "",
                            module: "propolis_server::initializer",
                        };
                        ::slog::RecordStatic {
                            location: &LOC,
                            level: ::slog::Level::Info,
                            tag: "",
                        }
                    };
                    ::slog::Record::new(
                        &RS,
                        &::core::fmt::Arguments::new_v1(
                            &["Creating in-memory disk from bytes"],
                            &match () {
                                _args => [],
                            },
                        ),
                        ::slog::BorrowedKV(&()),
                    )
                })
            };
            let be = propolis::block::InMemoryBackend::create(bytes, read_only, 512)?;
            if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                ::slog::Logger::log(&self.log, &{
                    #[allow(dead_code)]
                    static RS: ::slog::RecordStatic<'static> = {
                        static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                            file: "server/src/lib/initializer.rs",
                            line: 312u32,
                            column: 9u32,
                            function: "",
                            module: "propolis_server::initializer",
                        };
                        ::slog::RecordStatic {
                            location: &LOC,
                            level: ::slog::Level::Info,
                            tag: "",
                        }
                    };
                    ::slog::Record::new(
                        &RS,
                        &::core::fmt::Arguments::new_v1(
                            &["Creating ChildRegister"],
                            &match () {
                                _args => [],
                            },
                        ),
                        ::slog::BorrowedKV(&()),
                    )
                })
            };
            let creg = ChildRegister::new(&be, None);
            if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                ::slog::Logger::log(&self.log, &{
                    #[allow(dead_code)]
                    static RS: ::slog::RecordStatic<'static> = {
                        static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                            file: "server/src/lib/initializer.rs",
                            line: 315u32,
                            column: 9u32,
                            function: "",
                            module: "propolis_server::initializer",
                        };
                        ::slog::RecordStatic {
                            location: &LOC,
                            level: ::slog::Level::Info,
                            tag: "",
                        }
                    };
                    ::slog::Record::new(
                        &RS,
                        &::core::fmt::Arguments::new_v1(
                            &["Calling initialize_virtio_block"],
                            &match () {
                                _args => [],
                            },
                        ),
                        ::slog::BorrowedKV(&()),
                    )
                })
            };
            self.initialize_virtio_block(chipset, bdf, be, creg)
        }
        pub fn initialize_fwcfg(&self, cpus: u8) -> Result<crate::vnc::server::RamFb, Error> {
            let mut fwcfg = fwcfg::FwCfgBuilder::new();
            fwcfg
                .add_legacy(
                    fwcfg::LegacyId::SmpCpuCount,
                    fwcfg::FixedItem::new_u32(cpus as u32),
                )
                .unwrap();
            let ramfb = ramfb::RamFb::create();
            ramfb.attach(&mut fwcfg);
            let fwcfg_dev = fwcfg.finalize();
            let pio = self.mctx.pio();
            fwcfg_dev.attach(pio);
            self.inv.register(&fwcfg_dev)?;
            self.inv.register(&ramfb)?;
            let (addr, w, h) = ramfb.get_fb_info();
            let fb = crate::vnc::server::RamFb::new(addr, w as usize, h as usize);
            Ok(fb)
        }
        pub fn initialize_cpus(&self) -> Result<(), Error> {
            for mut vcpu in self.mctx.vcpus() {
                vcpu.set_default_capabs().unwrap();
            }
            Ok(())
        }
    }
}
mod migrate {
    use std::sync::Arc;
    use bit_field::BitField;
    use dropshot::{HttpError, RequestContext};
    use hyper::{header, Body, Method, Response, StatusCode};
    use propolis::{
        dispatch::AsyncCtx,
        instance::{Instance, MigratePhase, MigrateRole, State, TransitionError},
        migrate::MigrateStateError,
    };
    use propolis_client::api::{self, MigrationState};
    use serde::{Deserialize, Serialize};
    use slog::{error, info, o};
    use thiserror::Error;
    use tokio::{sync::RwLock, task::JoinHandle};
    use uuid::Uuid;
    use crate::server::Context;
    mod codec {
        //! Copyright 2021 Oxide Computer Company
        //!
        //! Support for framing messages in the propolis/bhyve live
        //! migration protocol.  Frames are defined by a 5-byte header
        //! consisting of a 32-bit length (unsigned little endian)
        //! followed by a tag byte indicating the frame type, and then
        //! the frame data.  The length field includes the header.
        //!
        //! As defined in RFD0071, most messages are either serialized
        //! structures or blobs, while the structures involved in the
        //! memory transfer phases of the protocols are directly serialized
        //! binary structures.  We represent each of these structures in a
        //! dedicated message type; similarly with 4KiB "page" data, etc.
        //! Serialized structures are assumed to be text.
        //!
        //! Several messages involved in memory transfer include bitmaps
        //! that are nominally bounded by associated [start, end) address
        //! ranges.  However, the framing layer makes no effort to validate
        //! the implied invariants: higher level software is responsible
        //! for that.
        use super::MigrateError;
        use bytes::{Buf, BufMut, BytesMut};
        use num_enum::{IntoPrimitive, TryFromPrimitive};
        use slog::error;
        use std::convert::TryFrom;
        use thiserror::Error;
        use tokio_util::codec;
        /// Migration protocol errors.
        pub enum ProtocolError {
            /// We received an unexpected message type
            #[error("couldn't decode message type ({0})")]
            InvalidMessageType(u8),
            /// The message received on the wire wasn't the expected length
            #[error("unexpected message length")]
            UnexpectedMessageLen,
            /// Encountered an I/O error on the transport
            #[error("I/O error: {0}")]
            Io(#[from] std::io::Error),
            /// Failed to serialize or deserialize a message
            #[error("serialization error: {0}")]
            Ron(#[from] ron::Error),
            /// Received non-UTF8 string
            #[error("non-UTF8 string: {0}")]
            Utf8(#[from] std::str::Utf8Error),
        }
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for ProtocolError {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match (&*self,) {
                    (&ProtocolError::InvalidMessageType(ref __self_0),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "InvalidMessageType");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&ProtocolError::UnexpectedMessageLen,) => {
                        ::core::fmt::Formatter::write_str(f, "UnexpectedMessageLen")
                    }
                    (&ProtocolError::Io(ref __self_0),) => {
                        let debug_trait_builder = &mut ::core::fmt::Formatter::debug_tuple(f, "Io");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&ProtocolError::Ron(ref __self_0),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "Ron");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&ProtocolError::Utf8(ref __self_0),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "Utf8");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                }
            }
        }
        #[allow(unused_qualifications)]
        impl std::error::Error for ProtocolError {
            fn source(&self) -> std::option::Option<&(dyn std::error::Error + 'static)> {
                use thiserror::private::AsDynError;
                #[allow(deprecated)]
                match self {
                    ProtocolError::InvalidMessageType { .. } => std::option::Option::None,
                    ProtocolError::UnexpectedMessageLen { .. } => std::option::Option::None,
                    ProtocolError::Io { 0: source, .. } => {
                        std::option::Option::Some(source.as_dyn_error())
                    }
                    ProtocolError::Ron { 0: source, .. } => {
                        std::option::Option::Some(source.as_dyn_error())
                    }
                    ProtocolError::Utf8 { 0: source, .. } => {
                        std::option::Option::Some(source.as_dyn_error())
                    }
                }
            }
        }
        #[allow(unused_qualifications)]
        impl std::fmt::Display for ProtocolError {
            fn fmt(&self, __formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                #[allow(unused_imports)]
                use thiserror::private::{DisplayAsDisplay, PathAsDisplay};
                #[allow(unused_variables, deprecated, clippy::used_underscore_binding)]
                match self {
                    ProtocolError::InvalidMessageType(_0) => {
                        __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                            &["couldn\'t decode message type (", ")"],
                            &match (&_0.as_display(),) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ))
                    }
                    ProtocolError::UnexpectedMessageLen {} => {
                        __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                            &["unexpected message length"],
                            &match () {
                                _args => [],
                            },
                        ))
                    }
                    ProtocolError::Io(_0) => __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["I/O error: "],
                        &match (&_0.as_display(),) {
                            _args => [::core::fmt::ArgumentV1::new(
                                _args.0,
                                ::core::fmt::Display::fmt,
                            )],
                        },
                    )),
                    ProtocolError::Ron(_0) => {
                        __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                            &["serialization error: "],
                            &match (&_0.as_display(),) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ))
                    }
                    ProtocolError::Utf8(_0) => {
                        __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                            &["non-UTF8 string: "],
                            &match (&_0.as_display(),) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ))
                    }
                }
            }
        }
        #[allow(unused_qualifications)]
        impl std::convert::From<std::io::Error> for ProtocolError {
            #[allow(deprecated)]
            fn from(source: std::io::Error) -> Self {
                ProtocolError::Io { 0: source }
            }
        }
        #[allow(unused_qualifications)]
        impl std::convert::From<ron::Error> for ProtocolError {
            #[allow(deprecated)]
            fn from(source: ron::Error) -> Self {
                ProtocolError::Ron { 0: source }
            }
        }
        #[allow(unused_qualifications)]
        impl std::convert::From<std::str::Utf8Error> for ProtocolError {
            #[allow(deprecated)]
            fn from(source: std::str::Utf8Error) -> Self {
                ProtocolError::Utf8 { 0: source }
            }
        }
        /// Message represents the different frame types for messages
        /// exchanged in the live migration protocol.  Most structured
        /// data is serialized into a string, while blobs are uninterpreted
        /// vectors of bytes and 4KiB pages (e.g. of RAM) are uninterpreted
        /// fixed-sized arrays.  The memory-related messages are nominally
        /// structured, but given the overall volume of memory data exchanged,
        /// we serialize and deserialize them directly.
        pub(crate) enum Message {
            Okay,
            Error(MigrateError),
            Serialized(String),
            Blob(Vec<u8>),
            Page(Vec<u8>),
            MemQuery(u64, u64),
            MemOffer(u64, u64, Vec<u8>),
            MemEnd(u64, u64),
            MemFetch(u64, u64, Vec<u8>),
            MemXfer(u64, u64, Vec<u8>),
            MemDone,
        }
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for Message {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match (&*self,) {
                    (&Message::Okay,) => ::core::fmt::Formatter::write_str(f, "Okay"),
                    (&Message::Error(ref __self_0),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "Error");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::Serialized(ref __self_0),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "Serialized");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::Blob(ref __self_0),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "Blob");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::Page(ref __self_0),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "Page");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::MemQuery(ref __self_0, ref __self_1),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "MemQuery");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::MemOffer(ref __self_0, ref __self_1, ref __self_2),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "MemOffer");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_2));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::MemEnd(ref __self_0, ref __self_1),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "MemEnd");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::MemFetch(ref __self_0, ref __self_1, ref __self_2),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "MemFetch");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_2));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::MemXfer(ref __self_0, ref __self_1, ref __self_2),) => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_tuple(f, "MemXfer");
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                        let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_2));
                        ::core::fmt::DebugTuple::finish(debug_trait_builder)
                    }
                    (&Message::MemDone,) => ::core::fmt::Formatter::write_str(f, "MemDone"),
                }
            }
        }
        /// MessageType represents tags that are used in the protocol for
        /// identifying frame types.  They are an implementation detail of
        /// the wire format, and not used elsewhere.  However, they must be
        /// kept in bijection with Message, above.
        #[repr(u8)]
        enum MessageType {
            Okay,
            Error,
            Serialized,
            Blob,
            Page,
            MemQuery,
            MemOffer,
            MemEnd,
            MemFetch,
            MemXfer,
            MemDone,
        }
        impl ::core::marker::StructuralPartialEq for MessageType {}
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::cmp::PartialEq for MessageType {
            #[inline]
            fn eq(&self, other: &MessageType) -> bool {
                {
                    let __self_vi = ::core::intrinsics::discriminant_value(&*self);
                    let __arg_1_vi = ::core::intrinsics::discriminant_value(&*other);
                    if true && __self_vi == __arg_1_vi {
                        match (&*self, &*other) {
                            _ => true,
                        }
                    } else {
                        false
                    }
                }
            }
        }
        impl From<MessageType> for u8 {
            #[inline]
            fn from(enum_value: MessageType) -> Self {
                enum_value as Self
            }
        }
        impl ::num_enum::TryFromPrimitive for MessageType {
            type Primitive = u8;
            const NAME: &'static str = "MessageType";
            fn try_from_primitive(
                number: Self::Primitive,
            ) -> ::core::result::Result<Self, ::num_enum::TryFromPrimitiveError<Self>> {
                #![allow(non_upper_case_globals)]
                const Okay__num_enum_0__: u8 = 0;
                const Error__num_enum_0__: u8 = u8::wrapping_add(0, 1);
                const Serialized__num_enum_0__: u8 = u8::wrapping_add(u8::wrapping_add(0, 1), 1);
                const Blob__num_enum_0__: u8 =
                    u8::wrapping_add(u8::wrapping_add(u8::wrapping_add(0, 1), 1), 1);
                const Page__num_enum_0__: u8 = u8::wrapping_add(
                    u8::wrapping_add(u8::wrapping_add(u8::wrapping_add(0, 1), 1), 1),
                    1,
                );
                const MemQuery__num_enum_0__: u8 = u8::wrapping_add(
                    u8::wrapping_add(
                        u8::wrapping_add(u8::wrapping_add(u8::wrapping_add(0, 1), 1), 1),
                        1,
                    ),
                    1,
                );
                const MemOffer__num_enum_0__: u8 = u8::wrapping_add(
                    u8::wrapping_add(
                        u8::wrapping_add(
                            u8::wrapping_add(u8::wrapping_add(u8::wrapping_add(0, 1), 1), 1),
                            1,
                        ),
                        1,
                    ),
                    1,
                );
                const MemEnd__num_enum_0__: u8 = u8::wrapping_add(
                    u8::wrapping_add(
                        u8::wrapping_add(
                            u8::wrapping_add(
                                u8::wrapping_add(u8::wrapping_add(u8::wrapping_add(0, 1), 1), 1),
                                1,
                            ),
                            1,
                        ),
                        1,
                    ),
                    1,
                );
                const MemFetch__num_enum_0__: u8 = u8::wrapping_add(
                    u8::wrapping_add(
                        u8::wrapping_add(
                            u8::wrapping_add(
                                u8::wrapping_add(
                                    u8::wrapping_add(
                                        u8::wrapping_add(u8::wrapping_add(0, 1), 1),
                                        1,
                                    ),
                                    1,
                                ),
                                1,
                            ),
                            1,
                        ),
                        1,
                    ),
                    1,
                );
                const MemXfer__num_enum_0__: u8 = u8::wrapping_add(
                    u8::wrapping_add(
                        u8::wrapping_add(
                            u8::wrapping_add(
                                u8::wrapping_add(
                                    u8::wrapping_add(
                                        u8::wrapping_add(
                                            u8::wrapping_add(u8::wrapping_add(0, 1), 1),
                                            1,
                                        ),
                                        1,
                                    ),
                                    1,
                                ),
                                1,
                            ),
                            1,
                        ),
                        1,
                    ),
                    1,
                );
                const MemDone__num_enum_0__: u8 = u8::wrapping_add(
                    u8::wrapping_add(
                        u8::wrapping_add(
                            u8::wrapping_add(
                                u8::wrapping_add(
                                    u8::wrapping_add(
                                        u8::wrapping_add(
                                            u8::wrapping_add(
                                                u8::wrapping_add(u8::wrapping_add(0, 1), 1),
                                                1,
                                            ),
                                            1,
                                        ),
                                        1,
                                    ),
                                    1,
                                ),
                                1,
                            ),
                            1,
                        ),
                        1,
                    ),
                    1,
                );
                #[deny(unreachable_patterns)]
                match number {
                    Okay__num_enum_0__ => ::core::result::Result::Ok(Self::Okay),
                    Error__num_enum_0__ => ::core::result::Result::Ok(Self::Error),
                    Serialized__num_enum_0__ => ::core::result::Result::Ok(Self::Serialized),
                    Blob__num_enum_0__ => ::core::result::Result::Ok(Self::Blob),
                    Page__num_enum_0__ => ::core::result::Result::Ok(Self::Page),
                    MemQuery__num_enum_0__ => ::core::result::Result::Ok(Self::MemQuery),
                    MemOffer__num_enum_0__ => ::core::result::Result::Ok(Self::MemOffer),
                    MemEnd__num_enum_0__ => ::core::result::Result::Ok(Self::MemEnd),
                    MemFetch__num_enum_0__ => ::core::result::Result::Ok(Self::MemFetch),
                    MemXfer__num_enum_0__ => ::core::result::Result::Ok(Self::MemXfer),
                    MemDone__num_enum_0__ => ::core::result::Result::Ok(Self::MemDone),
                    #[allow(unreachable_patterns)]
                    _ => ::core::result::Result::Err(::num_enum::TryFromPrimitiveError { number }),
                }
            }
        }
        impl ::core::convert::TryFrom<u8> for MessageType {
            type Error = ::num_enum::TryFromPrimitiveError<Self>;
            #[inline]
            fn try_from(
                number: u8,
            ) -> ::core::result::Result<Self, ::num_enum::TryFromPrimitiveError<Self>> {
                ::num_enum::TryFromPrimitive::try_from_primitive(number)
            }
        }
        /// By implementing `From<&Message>` on MessageType, we can translate
        /// each message into its tag type, ensuring full coverage.
        impl From<&Message> for MessageType {
            fn from(m: &Message) -> MessageType {
                match m {
                    Message::Okay => MessageType::Okay,
                    Message::Error(_) => MessageType::Error,
                    Message::Serialized(_) => MessageType::Serialized,
                    Message::Blob(_) => MessageType::Blob,
                    Message::Page(_) => MessageType::Page,
                    Message::MemQuery(_, _) => MessageType::MemQuery,
                    Message::MemOffer(_, _, _) => MessageType::MemOffer,
                    Message::MemEnd(_, _) => MessageType::MemEnd,
                    Message::MemFetch(_, _, _) => MessageType::MemFetch,
                    Message::MemXfer(_, _, _) => MessageType::MemXfer,
                    Message::MemDone => MessageType::MemDone,
                }
            }
        }
        /// `LiveMigrationEncoder` implements the `Encoder` & `Decoder`
        /// traits for transforming a stream of bytes to/from migration
        /// protocol messages.
        pub(crate) struct LiveMigrationFramer {
            log: slog::Logger,
        }
        impl LiveMigrationFramer {
            /// Creates a new LiveMigrationFramer, which represents the
            /// right to encode and decode messages.
            pub fn new(log: slog::Logger) -> LiveMigrationFramer {
                LiveMigrationFramer { log }
            }
            /// Writes the header at the start of the frame.  Also reserves enough space
            /// in the destination buffer for the complete message.
            fn put_header(&mut self, tag: MessageType, len: usize, dst: &mut BytesMut) {
                let len = len + 5;
                if dst.remaining_mut() < len {
                    dst.reserve(len - dst.remaining_mut());
                }
                dst.put_u32_le(len as u32);
                dst.put_u8(tag.into());
            }
            fn put_start_end(&mut self, start: u64, end: u64, dst: &mut BytesMut) {
                dst.put_u64_le(start);
                dst.put_u64_le(end);
            }
            fn put_bitmap(&mut self, bitmap: &[u8], dst: &mut BytesMut) {
                dst.put(bitmap);
            }
            fn get_start_end(
                &mut self,
                len: usize,
                src: &mut BytesMut,
            ) -> Result<(usize, u64, u64), ProtocolError> {
                if len < 16 {
                    if ::slog::Level::Error.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log, &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/codec.rs",
                                    line: 160u32,
                                    column: 13u32,
                                    function: "",
                                    module: "propolis_server::migrate::codec",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Error,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["short message reading start end: "],
                                    &match (&len,) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Display::fmt,
                                        )],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    return Err(ProtocolError::UnexpectedMessageLen);
                }
                let start = src.get_u64_le();
                let end = src.get_u64_le();
                Ok((len - 16, start, end))
            }
            fn get_bitmap(
                &mut self,
                len: usize,
                src: &mut BytesMut,
            ) -> Result<Vec<u8>, ProtocolError> {
                let remaining = src.remaining();
                if remaining < len {
                    if ::slog::Level::Error.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log, &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/codec.rs",
                                    line: 177u32,
                                    column: 13u32,
                                    function: "",
                                    module: "propolis_server::migrate::codec",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Error,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["short message reading bitmap (remaining: ", " len: "],
                                    &match (&remaining, &len) {
                                        _args => [
                                            ::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Display::fmt,
                                            ),
                                            ::core::fmt::ArgumentV1::new(
                                                _args.1,
                                                ::core::fmt::Display::fmt,
                                            ),
                                        ],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    return Err(ProtocolError::UnexpectedMessageLen);
                }
                let v = src[..len].to_vec();
                src.advance(len);
                Ok(v.to_vec())
            }
        }
        impl codec::Encoder<Message> for LiveMigrationFramer {
            type Error = ProtocolError;
            fn encode(&mut self, m: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
                let tag = (&m).into();
                match m {
                    Message::Okay => {
                        self.put_header(tag, 0, dst);
                    }
                    Message::Error(e) => {
                        let serialized = ron::ser::to_string(&e)?;
                        let bytes = serialized.into_bytes();
                        self.put_header(tag, bytes.len(), dst);
                        dst.put(&bytes[..]);
                    }
                    Message::Serialized(s) => {
                        let bytes = s.into_bytes();
                        self.put_header(tag, bytes.len(), dst);
                        dst.put(&bytes[..]);
                    }
                    Message::Blob(bytes) => {
                        self.put_header(tag, bytes.len(), dst);
                        dst.put(&bytes[..]);
                    }
                    Message::Page(page) => {
                        self.put_header(tag, page.len(), dst);
                        dst.put(&page[..]);
                    }
                    Message::MemQuery(start, end) => {
                        self.put_header(tag, 8 + 8, dst);
                        self.put_start_end(start, end, dst);
                    }
                    Message::MemOffer(start, end, bitmap) => {
                        self.put_header(tag, 8 + 8 + bitmap.len(), dst);
                        self.put_start_end(start, end, dst);
                        self.put_bitmap(&bitmap, dst);
                    }
                    Message::MemEnd(start, end) => {
                        self.put_header(tag, 8 + 8, dst);
                        self.put_start_end(start, end, dst);
                    }
                    Message::MemFetch(start, end, bitmap) => {
                        self.put_header(tag, 8 + 8 + bitmap.len(), dst);
                        self.put_start_end(start, end, dst);
                        self.put_bitmap(&bitmap, dst);
                    }
                    Message::MemXfer(start, end, bitmap) => {
                        self.put_header(tag, 8 + 8 + bitmap.len(), dst);
                        self.put_start_end(start, end, dst);
                        self.put_bitmap(&bitmap, dst);
                    }
                    Message::MemDone => {
                        self.put_header(tag, 0, dst);
                    }
                };
                Ok(())
            }
        }
        impl codec::Decoder for LiveMigrationFramer {
            type Item = Message;
            type Error = ProtocolError;
            fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
                if src.remaining() < 5 {
                    return Ok(None);
                }
                let tag = MessageType::try_from(src[4])
                    .map_err(|_| ProtocolError::InvalidMessageType(src[4]))?;
                let len = u32::from_le_bytes([src[0], src[1], src[2], src[3]]) as usize;
                if len < 5 {
                    if ::slog::Level::Error.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log, &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/codec.rs",
                                    line: 274u32,
                                    column: 13u32,
                                    function: "",
                                    module: "propolis_server::migrate::codec",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Error,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["decode: length too short for header "],
                                    &match (&len,) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Display::fmt,
                                        )],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    return Err(ProtocolError::UnexpectedMessageLen);
                }
                if src.remaining() < len {
                    src.reserve(len - src.remaining());
                    return Ok(None);
                }
                src.advance(5);
                let len = len - 5;
                let m = match tag {
                    MessageType::Okay => {
                        {
                            match (&len, &0) {
                                (left_val, right_val) => {
                                    if !(*left_val == *right_val) {
                                        let kind = ::core::panicking::AssertKind::Eq;
                                        ::core::panicking::assert_failed(
                                            kind,
                                            &*left_val,
                                            &*right_val,
                                            ::core::option::Option::None,
                                        );
                                    }
                                }
                            }
                        };
                        Message::Okay
                    }
                    MessageType::Error => {
                        let e = ron::de::from_str(std::str::from_utf8(&src[..len])?)?;
                        src.advance(len);
                        Message::Error(e)
                    }
                    MessageType::Serialized => {
                        let s = std::str::from_utf8(&src[..len])?.to_string();
                        src.advance(len);
                        Message::Serialized(s)
                    }
                    MessageType::Blob => {
                        let v = src[..len].to_vec();
                        src.advance(len);
                        Message::Blob(v)
                    }
                    MessageType::Page => {
                        if len != 4096 {
                            if ::slog::Level::Error.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&self.log, &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/migrate/codec.rs",
                                                line: 310u32,
                                                column: 21u32,
                                                function: "",
                                                module: "propolis_server::migrate::codec",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Error,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["decode: invalid length for `Page` message (len)"],
                                            &match () {
                                                _args => [],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                            return Err(ProtocolError::UnexpectedMessageLen);
                        }
                        let p = src[..len].to_vec();
                        src.advance(len);
                        Message::Page(p)
                    }
                    MessageType::MemQuery => {
                        let (_, start, end) = self.get_start_end(len, src)?;
                        Message::MemQuery(start, end)
                    }
                    MessageType::MemOffer => {
                        let (len, start, end) = self.get_start_end(len, src)?;
                        let bitmap = self.get_bitmap(len, src)?;
                        Message::MemOffer(start, end, bitmap)
                    }
                    MessageType::MemEnd => {
                        let (_, start, end) = self.get_start_end(len, src)?;
                        Message::MemEnd(start, end)
                    }
                    MessageType::MemFetch => {
                        let (len, start, end) = self.get_start_end(len, src)?;
                        let bitmap = self.get_bitmap(len, src)?;
                        Message::MemFetch(start, end, bitmap)
                    }
                    MessageType::MemXfer => {
                        let (len, start, end) = self.get_start_end(len, src)?;
                        let bitmap = self.get_bitmap(len, src)?;
                        Message::MemXfer(start, end, bitmap)
                    }
                    MessageType::MemDone => {
                        {
                            match (&len, &0) {
                                (left_val, right_val) => {
                                    if !(*left_val == *right_val) {
                                        let kind = ::core::panicking::AssertKind::Eq;
                                        ::core::panicking::assert_failed(
                                            kind,
                                            &*left_val,
                                            &*right_val,
                                            ::core::option::Option::None,
                                        );
                                    }
                                }
                            }
                        };
                        Message::MemDone
                    }
                };
                Ok(Some(m))
            }
        }
    }
    mod destination {
        use bitvec::prelude as bv;
        use futures::{SinkExt, StreamExt};
        use hyper::upgrade::Upgraded;
        use propolis::common::GuestAddr;
        use propolis::instance::MigrateRole;
        use propolis::migrate::{MigrateStateError, Migrator};
        use slog::{error, info, warn};
        use std::io;
        use std::sync::Arc;
        use tokio_util::codec::Framed;
        use crate::migrate::codec::{self, LiveMigrationFramer};
        use crate::migrate::memx;
        use crate::migrate::preamble::Preamble;
        use crate::migrate::{Device, MigrateContext, MigrateError, MigrationState, PageIter};
        pub async fn migrate(
            mctx: Arc<MigrateContext>,
            conn: Upgraded,
        ) -> Result<(), MigrateError> {
            let mut proto = DestinationProtocol::new(mctx, conn);
            if let Err(err) = proto.run().await {
                proto.mctx.set_state(MigrationState::Error).await;
                let _ = proto.conn.send(codec::Message::Error(err.clone())).await;
                return Err(err);
            }
            Ok(())
        }
        struct DestinationProtocol {
            /// The migration context which also contains the `Instance` handle.
            mctx: Arc<MigrateContext>,
            /// Transport to the source Instance.
            conn: Framed<Upgraded, LiveMigrationFramer>,
        }
        impl DestinationProtocol {
            fn new(mctx: Arc<MigrateContext>, conn: Upgraded) -> Self {
                let codec_log = mctx.log.new(::slog::OwnedKV(()));
                Self {
                    mctx,
                    conn: Framed::new(conn, LiveMigrationFramer::new(codec_log)),
                }
            }
            fn log(&self) -> &slog::Logger {
                &self.mctx.log
            }
            async fn run(&mut self) -> Result<(), MigrateError> {
                self.start();
                self.sync().await?;
                self.ram_push().await?;
                self.device_state().await?;
                self.arch_state().await?;
                self.ram_pull().await?;
                self.finish().await?;
                self.end();
                Ok(())
            }
            fn start(&mut self) {
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/destination.rs",
                                line: 72u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::destination",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["Entering Destination Migration Task"],
                                &match () {
                                    _args => [],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
            }
            async fn sync(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Sync).await;
                let preamble: Preamble = match self.read_msg().await? {
                    codec::Message::Serialized(s) => {
                        Ok(ron::de::from_str(&s).map_err(codec::ProtocolError::from)?)
                    }
                    msg => {
                        if ::slog::Level::Error.as_usize()
                            <= ::slog::__slog_static_max_level().as_usize()
                        {
                            ::slog::Logger::log(&self.log(), &{
                                #[allow(dead_code)]
                                static RS: ::slog::RecordStatic<'static> = {
                                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                        file: "server/src/lib/migrate/destination.rs",
                                        line: 82u32,
                                        column: 17u32,
                                        function: "",
                                        module: "propolis_server::migrate::destination",
                                    };
                                    ::slog::RecordStatic {
                                        location: &LOC,
                                        level: ::slog::Level::Error,
                                        tag: "",
                                    }
                                };
                                ::slog::Record::new(
                                    &RS,
                                    &::core::fmt::Arguments::new_v1(
                                        &["expected serialized preamble but received: "],
                                        &match (&msg,) {
                                            _args => [::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Debug::fmt,
                                            )],
                                        },
                                    ),
                                    ::slog::BorrowedKV(&()),
                                )
                            })
                        };
                        Err(MigrateError::UnexpectedMessage)
                    }
                }?;
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/destination.rs",
                                line: 89u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::destination",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["Src read Preamble: "],
                                &match (&preamble,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Debug::fmt,
                                    )],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                if preamble.vm_descr.vcpus != <[_]>::into_vec(box [0u32, 1, 2, 3]) {
                    if ::slog::Level::Error.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log(), &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/destination.rs",
                                    line: 92u32,
                                    column: 13u32,
                                    function: "",
                                    module: "propolis_server::migrate::destination",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Error,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["invalid CPU count in preamble (", ")"],
                                    &match (&preamble.vm_descr.vcpus,) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Debug::fmt,
                                        )],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    return Err(MigrateError::InvalidInstanceState);
                }
                self.send_msg(codec::Message::Okay).await
            }
            async fn ram_push(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::RamPush).await;
                let (dirty, highest) = self.query_ram().await?;
                for (k, region) in dirty.as_raw_slice().chunks(4096).enumerate() {
                    if region.iter().all(|&b| b == 0) {
                        continue;
                    }
                    let start = (k * 4096 * 8 * 4096) as u64;
                    let end = start + (region.len() * 8 * 4096) as u64;
                    let end = highest.min(end);
                    self.send_msg(memx::make_mem_fetch(start, end, region))
                        .await?;
                    let m = self.read_msg().await?;
                    if ::slog::Level::Info.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log(), &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/destination.rs",
                                    line: 113u32,
                                    column: 13u32,
                                    function: "",
                                    module: "propolis_server::migrate::destination",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Info,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["ram_push: source xfer phase recvd "],
                                    &match (&m,) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Debug::fmt,
                                        )],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    match m {
                        codec::Message::MemXfer(start, end, bits) => {
                            if !memx::validate_bitmap(start, end, &bits) {
                                if ::slog::Level::Error.as_usize()
                                    <= ::slog::__slog_static_max_level().as_usize()
                                {
                                    ::slog::Logger::log(&self.log(), &{
                                        #[allow(dead_code)]
                                        static RS: ::slog::RecordStatic<'static> = {
                                            static LOC: ::slog::RecordLocation =
                                                ::slog::RecordLocation {
                                                    file: "server/src/lib/migrate/destination.rs",
                                                    line: 117u32,
                                                    column: 25u32,
                                                    function: "",
                                                    module: "propolis_server::migrate::destination",
                                                };
                                            ::slog::RecordStatic {
                                                location: &LOC,
                                                level: ::slog::Level::Error,
                                                tag: "",
                                            }
                                        };
                                        ::slog::Record::new(
                                            &RS,
                                            &::core::fmt::Arguments::new_v1(
                                                &["ram_push: MemXfer received bad bitmap"],
                                                &match () {
                                                    _args => [],
                                                },
                                            ),
                                            ::slog::BorrowedKV(&()),
                                        )
                                    })
                                };
                                return Err(MigrateError::Phase);
                            }
                            self.xfer_ram(start, end, &bits).await?;
                        }
                        _ => return Err(MigrateError::UnexpectedMessage),
                    };
                }
                self.send_msg(codec::Message::MemDone).await?;
                self.mctx.set_state(MigrationState::Pause).await;
                Ok(())
            }
            async fn query_ram(&mut self) -> Result<(bv::BitVec<u8, bv::Lsb0>, u64), MigrateError> {
                self.send_msg(codec::Message::MemQuery(0, !0)).await?;
                let mut dirty = bv::BitVec::<u8, bv::Lsb0>::new();
                let mut highest = 0;
                loop {
                    let m = self.read_msg().await?;
                    if ::slog::Level::Info.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log(), &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/destination.rs",
                                    line: 147u32,
                                    column: 13u32,
                                    function: "",
                                    module: "propolis_server::migrate::destination",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Info,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["ram_push: source xfer phase recvd "],
                                    &match (&m,) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Debug::fmt,
                                        )],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    match m {
                        codec::Message::MemEnd(start, end) => {
                            if start != 0 || end != !0 {
                                if ::slog::Level::Error.as_usize()
                                    <= ::slog::__slog_static_max_level().as_usize()
                                {
                                    ::slog::Logger::log(&self.log(), &{
                                        #[allow(dead_code)]
                                        static RS: ::slog::RecordStatic<'static> = {
                                            static LOC: ::slog::RecordLocation =
                                                ::slog::RecordLocation {
                                                    file: "server/src/lib/migrate/destination.rs",
                                                    line: 151u32,
                                                    column: 25u32,
                                                    function: "",
                                                    module: "propolis_server::migrate::destination",
                                                };
                                            ::slog::RecordStatic {
                                                location: &LOC,
                                                level: ::slog::Level::Error,
                                                tag: "",
                                            }
                                        };
                                        ::slog::Record::new(
                                            &RS,
                                            &::core::fmt::Arguments::new_v1(
                                                &["ram_push: received bad MemEnd"],
                                                &match () {
                                                    _args => [],
                                                },
                                            ),
                                            ::slog::BorrowedKV(&()),
                                        )
                                    })
                                };
                                return Err(MigrateError::Phase);
                            }
                            break;
                        }
                        codec::Message::MemOffer(start, end, bits) => {
                            if !memx::validate_bitmap(start, end, &bits) {
                                if ::slog::Level::Error.as_usize()
                                    <= ::slog::__slog_static_max_level().as_usize()
                                {
                                    ::slog::Logger::log(&self.log(), &{
                                        #[allow(dead_code)]
                                        static RS: ::slog::RecordStatic<'static> = {
                                            static LOC: ::slog::RecordLocation =
                                                ::slog::RecordLocation {
                                                    file: "server/src/lib/migrate/destination.rs",
                                                    line: 158u32,
                                                    column: 25u32,
                                                    function: "",
                                                    module: "propolis_server::migrate::destination",
                                                };
                                            ::slog::RecordStatic {
                                                location: &LOC,
                                                level: ::slog::Level::Error,
                                                tag: "",
                                            }
                                        };
                                        ::slog::Record::new(
                                            &RS,
                                            &::core::fmt::Arguments::new_v1(
                                                &["ram_push: MemOffer received bad bitmap"],
                                                &match () {
                                                    _args => [],
                                                },
                                            ),
                                            ::slog::BorrowedKV(&()),
                                        )
                                    })
                                };
                                return Err(MigrateError::Phase);
                            }
                            if end > highest {
                                highest = end;
                            }
                            let start_bit_index = start as usize / 4096;
                            if dirty.len() < start_bit_index {
                                dirty.resize(start_bit_index, false);
                            }
                            dirty.extend_from_raw_slice(&bits);
                        }
                        _ => return Err(MigrateError::UnexpectedMessage),
                    }
                }
                Ok((dirty, highest))
            }
            async fn xfer_ram(
                &mut self,
                start: u64,
                end: u64,
                bits: &[u8],
            ) -> Result<(), MigrateError> {
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/destination.rs",
                                line: 185u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::destination",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["ram_push: xfer RAM between ", " and "],
                                &match (&start, &end) {
                                    _args => [
                                        ::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Display::fmt,
                                        ),
                                        ::core::fmt::ArgumentV1::new(
                                            _args.1,
                                            ::core::fmt::Display::fmt,
                                        ),
                                    ],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                for addr in PageIter::new(start, end, bits) {
                    let bytes = self.read_page().await?;
                    self.write_guest_ram(GuestAddr(addr), &bytes).await?;
                }
                Ok(())
            }
            async fn device_state(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Device).await;
                let devices: Vec<Device> = match self.read_msg().await? {
                    codec::Message::Serialized(encoded) => ron::de::from_reader(encoded.as_bytes())
                        .map_err(codec::ProtocolError::from)?,
                    msg => {
                        if ::slog::Level::Error.as_usize()
                            <= ::slog::__slog_static_max_level().as_usize()
                        {
                            ::slog::Logger::log(&self.log(), &{
                                #[allow(dead_code)]
                                static RS: ::slog::RecordStatic<'static> = {
                                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                        file: "server/src/lib/migrate/destination.rs",
                                        line: 202u32,
                                        column: 17u32,
                                        function: "",
                                        module: "propolis_server::migrate::destination",
                                    };
                                    ::slog::RecordStatic {
                                        location: &LOC,
                                        level: ::slog::Level::Error,
                                        tag: "",
                                    }
                                };
                                ::slog::Record::new(
                                    &RS,
                                    &::core::fmt::Arguments::new_v1(
                                        &["device_state: unexpected message: "],
                                        &match (&msg,) {
                                            _args => [::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Debug::fmt,
                                            )],
                                        },
                                    ),
                                    ::slog::BorrowedKV(&()),
                                )
                            })
                        };
                        return Err(MigrateError::UnexpectedMessage);
                    }
                };
                self.read_ok().await?;
                let dispctx = self
                    .mctx
                    .async_ctx
                    .dispctx()
                    .await
                    .ok_or_else(|| MigrateError::InstanceNotInitialized)?;
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/destination.rs",
                                line: 215u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::destination",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1_formatted(
                                &["Devices: "],
                                &match (&devices,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Debug::fmt,
                                    )],
                                },
                                &[::core::fmt::rt::v1::Argument {
                                    position: 0usize,
                                    format: ::core::fmt::rt::v1::FormatSpec {
                                        fill: ' ',
                                        align: ::core::fmt::rt::v1::Alignment::Unknown,
                                        flags: 4u32,
                                        precision: ::core::fmt::rt::v1::Count::Implied,
                                        width: ::core::fmt::rt::v1::Count::Implied,
                                    },
                                }],
                                unsafe { ::core::fmt::UnsafeArg::new() },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                let inv = self.mctx.instance.inv();
                for device in devices {
                    let dev_ent = inv
                        .get_by_name(&device.instance_name)
                        .ok_or_else(|| MigrateError::UnknownDevice(device.instance_name.clone()))?;
                    match dev_ent.migrate() {
                        Migrator::NonMigratable => {
                            if ::slog::Level::Error.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&self.log(), &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/migrate/destination.rs",
                                                line: 226u32,
                                                column: 21u32,
                                                function: "",
                                                module: "propolis_server::migrate::destination",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Error,
                                            tag: "",
                                        }
                                    };
                                    :: slog :: Record :: new (& RS , & :: core :: fmt :: Arguments :: new_v1 (& ["Can\'t migrate instance with non-migratable device (" , ")"] , & match (& device . instance_name ,) { _args => [:: core :: fmt :: ArgumentV1 :: new (_args . 0 , :: core :: fmt :: Display :: fmt)] , }) , :: slog :: BorrowedKV (& ()))
                                })
                            };
                            return Err(MigrateError::DeviceState(
                                MigrateStateError::NonMigratable,
                            ));
                        }
                        Migrator::Simple => {
                            if ::slog::Level::Warning.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&self.log(), &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/migrate/destination.rs",
                                                line: 233u32,
                                                column: 21u32,
                                                function: "",
                                                module: "propolis_server::migrate::destination",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Warning,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["received unexpected device state for device "],
                                            &match (&device.instance_name,) {
                                                _args => [::core::fmt::ArgumentV1::new(
                                                    _args.0,
                                                    ::core::fmt::Display::fmt,
                                                )],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                        }
                        Migrator::Custom(migrate) => {
                            let mut deserializer = ron::Deserializer::from_str(&device.payload)
                                .map_err(codec::ProtocolError::from)?;
                            let deserializer =
                                &mut <dyn erased_serde::Deserializer>::erase(&mut deserializer);
                            migrate.import(dev_ent.type_name(), deserializer, &dispctx)?;
                        }
                    }
                }
                drop(dispctx);
                self.send_msg(codec::Message::Okay).await
            }
            async fn arch_state(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Arch).await;
                self.send_msg(codec::Message::Okay).await?;
                self.read_ok().await
            }
            async fn ram_pull(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::RamPull).await;
                self.send_msg(codec::Message::MemQuery(0, !0)).await?;
                let m = self.read_msg().await?;
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/destination.rs",
                                line: 270u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::destination",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["ram_pull: got end "],
                                &match (&m,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Debug::fmt,
                                    )],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                self.send_msg(codec::Message::MemDone).await
            }
            async fn finish(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Finish).await;
                self.send_msg(codec::Message::Okay).await?;
                let _ = self.read_ok().await;
                Ok(())
            }
            fn end(&mut self) {
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/destination.rs",
                                line: 282u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::destination",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["Destination Migration Successful"],
                                &match () {
                                    _args => [],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
            }
            async fn read_msg(&mut self) -> Result<codec::Message, MigrateError> {
                self.conn
                    .next()
                    .await
                    .ok_or_else(|| {
                        codec::ProtocolError::Io(io::Error::from(io::ErrorKind::BrokenPipe))
                    })?
                    .map(|msg| match msg {
                        codec::Message::Error(err) => {
                            if ::slog::Level::Error.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&self.log(), &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/migrate/destination.rs",
                                                line: 297u32,
                                                column: 21u32,
                                                function: "",
                                                module: "propolis_server::migrate::destination",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Error,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["remote error: "],
                                            &match (&err,) {
                                                _args => [::core::fmt::ArgumentV1::new(
                                                    _args.0,
                                                    ::core::fmt::Display::fmt,
                                                )],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                            Err(MigrateError::RemoteError(
                                MigrateRole::Source,
                                err.to_string(),
                            ))
                        }
                        msg => Ok(msg),
                    })?
            }
            async fn read_ok(&mut self) -> Result<(), MigrateError> {
                match self.read_msg().await? {
                    codec::Message::Okay => Ok(()),
                    msg => {
                        if ::slog::Level::Error.as_usize()
                            <= ::slog::__slog_static_max_level().as_usize()
                        {
                            ::slog::Logger::log(&self.log(), &{
                                #[allow(dead_code)]
                                static RS: ::slog::RecordStatic<'static> = {
                                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                        file: "server/src/lib/migrate/destination.rs",
                                        line: 311u32,
                                        column: 17u32,
                                        function: "",
                                        module: "propolis_server::migrate::destination",
                                    };
                                    ::slog::RecordStatic {
                                        location: &LOC,
                                        level: ::slog::Level::Error,
                                        tag: "",
                                    }
                                };
                                ::slog::Record::new(
                                    &RS,
                                    &::core::fmt::Arguments::new_v1(
                                        &["expected `Okay` but received: "],
                                        &match (&msg,) {
                                            _args => [::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Debug::fmt,
                                            )],
                                        },
                                    ),
                                    ::slog::BorrowedKV(&()),
                                )
                            })
                        };
                        Err(MigrateError::UnexpectedMessage)
                    }
                }
            }
            async fn read_page(&mut self) -> Result<Vec<u8>, MigrateError> {
                match self.read_msg().await? {
                    codec::Message::Page(bytes) => Ok(bytes),
                    _ => Err(MigrateError::UnexpectedMessage),
                }
            }
            async fn send_msg(&mut self, m: codec::Message) -> Result<(), MigrateError> {
                Ok(self.conn.send(m).await?)
            }
            async fn write_guest_ram(
                &mut self,
                addr: GuestAddr,
                buf: &[u8],
            ) -> Result<(), MigrateError> {
                let memctx = self
                    .mctx
                    .async_ctx
                    .dispctx()
                    .await
                    .ok_or(MigrateError::InstanceNotInitialized)?
                    .mctx
                    .memctx();
                let len = buf.len();
                memctx.write_from(addr, buf, len);
                Ok(())
            }
        }
    }
    mod memx {
        use crate::migrate::codec;
        pub(crate) fn validate_bitmap(start: u64, end: u64, bits: &[u8]) -> bool {
            if start % 4096 != 0 || end % 4096 != 0 {
                return false;
            }
            if end <= start {
                return false;
            }
            let npages = ((end - start) / 4096) as usize;
            let npages_bitmap = bits.len() * 8;
            if npages_bitmap < npages || (npages_bitmap - npages) >= 8 {
                return false;
            }
            if npages_bitmap != npages {
                let last_bits = npages_bitmap - npages;
                let mask = !0u8 << (8 - last_bits);
                let last_byte = bits[bits.len() - 1];
                if last_byte & mask != 0 {
                    return false;
                }
            }
            true
        }
        /// Creates an offer message for a range of physical
        /// addresses and a bitmap.
        pub(crate) fn make_mem_offer(
            start_gpa: u64,
            end_gpa: u64,
            bitmap: &[u8],
        ) -> codec::Message {
            if !validate_bitmap(start_gpa, end_gpa, bitmap) {
                ::core::panicking::panic(
                    "assertion failed: validate_bitmap(start_gpa, end_gpa, bitmap)",
                )
            };
            codec::Message::MemOffer(start_gpa, end_gpa, bitmap.into())
        }
        pub(crate) fn make_mem_fetch(
            start_gpa: u64,
            end_gpa: u64,
            bitmap: &[u8],
        ) -> codec::Message {
            if !validate_bitmap(start_gpa, end_gpa, bitmap) {
                ::core::panicking::panic(
                    "assertion failed: validate_bitmap(start_gpa, end_gpa, bitmap)",
                )
            };
            codec::Message::MemFetch(start_gpa, end_gpa, bitmap.into())
        }
        pub(crate) fn make_mem_xfer(start_gpa: u64, end_gpa: u64, bitmap: &[u8]) -> codec::Message {
            if !validate_bitmap(start_gpa, end_gpa, bitmap) {
                ::core::panicking::panic(
                    "assertion failed: validate_bitmap(start_gpa, end_gpa, bitmap)",
                )
            };
            codec::Message::MemXfer(start_gpa, end_gpa, bitmap.into())
        }
    }
    mod preamble {
        use propolis::instance::Instance;
        use serde::{Deserialize, Serialize};
        pub(crate) enum MemType {
            RAM,
            ROM,
            Dev,
            Res,
        }
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl<'de> _serde::Deserialize<'de> for MemType {
                fn deserialize<__D>(
                    __deserializer: __D,
                ) -> _serde::__private::Result<Self, __D::Error>
                where
                    __D: _serde::Deserializer<'de>,
                {
                    #[allow(non_camel_case_types)]
                    enum __Field {
                        __field0,
                        __field1,
                        __field2,
                        __field3,
                    }
                    struct __FieldVisitor;
                    impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(
                                __formatter,
                                "variant identifier",
                            )
                        }
                        fn visit_u64<__E>(
                            self,
                            __value: u64,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                0u64 => _serde::__private::Ok(__Field::__field0),
                                1u64 => _serde::__private::Ok(__Field::__field1),
                                2u64 => _serde::__private::Ok(__Field::__field2),
                                3u64 => _serde::__private::Ok(__Field::__field3),
                                _ => _serde::__private::Err(_serde::de::Error::invalid_value(
                                    _serde::de::Unexpected::Unsigned(__value),
                                    &"variant index 0 <= i < 4",
                                )),
                            }
                        }
                        fn visit_str<__E>(
                            self,
                            __value: &str,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                "RAM" => _serde::__private::Ok(__Field::__field0),
                                "ROM" => _serde::__private::Ok(__Field::__field1),
                                "Dev" => _serde::__private::Ok(__Field::__field2),
                                "Res" => _serde::__private::Ok(__Field::__field3),
                                _ => _serde::__private::Err(_serde::de::Error::unknown_variant(
                                    __value, VARIANTS,
                                )),
                            }
                        }
                        fn visit_bytes<__E>(
                            self,
                            __value: &[u8],
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                b"RAM" => _serde::__private::Ok(__Field::__field0),
                                b"ROM" => _serde::__private::Ok(__Field::__field1),
                                b"Dev" => _serde::__private::Ok(__Field::__field2),
                                b"Res" => _serde::__private::Ok(__Field::__field3),
                                _ => {
                                    let __value = &_serde::__private::from_utf8_lossy(__value);
                                    _serde::__private::Err(_serde::de::Error::unknown_variant(
                                        __value, VARIANTS,
                                    ))
                                }
                            }
                        }
                    }
                    impl<'de> _serde::Deserialize<'de> for __Field {
                        #[inline]
                        fn deserialize<__D>(
                            __deserializer: __D,
                        ) -> _serde::__private::Result<Self, __D::Error>
                        where
                            __D: _serde::Deserializer<'de>,
                        {
                            _serde::Deserializer::deserialize_identifier(
                                __deserializer,
                                __FieldVisitor,
                            )
                        }
                    }
                    struct __Visitor<'de> {
                        marker: _serde::__private::PhantomData<MemType>,
                        lifetime: _serde::__private::PhantomData<&'de ()>,
                    }
                    impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                        type Value = MemType;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "enum MemType")
                        }
                        fn visit_enum<__A>(
                            self,
                            __data: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::EnumAccess<'de>,
                        {
                            match match _serde::de::EnumAccess::variant(__data) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                (__Field::__field0, __variant) => {
                                    match _serde::de::VariantAccess::unit_variant(__variant) {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    };
                                    _serde::__private::Ok(MemType::RAM)
                                }
                                (__Field::__field1, __variant) => {
                                    match _serde::de::VariantAccess::unit_variant(__variant) {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    };
                                    _serde::__private::Ok(MemType::ROM)
                                }
                                (__Field::__field2, __variant) => {
                                    match _serde::de::VariantAccess::unit_variant(__variant) {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    };
                                    _serde::__private::Ok(MemType::Dev)
                                }
                                (__Field::__field3, __variant) => {
                                    match _serde::de::VariantAccess::unit_variant(__variant) {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    };
                                    _serde::__private::Ok(MemType::Res)
                                }
                            }
                        }
                    }
                    const VARIANTS: &'static [&'static str] = &["RAM", "ROM", "Dev", "Res"];
                    _serde::Deserializer::deserialize_enum(
                        __deserializer,
                        "MemType",
                        VARIANTS,
                        __Visitor {
                            marker: _serde::__private::PhantomData::<MemType>,
                            lifetime: _serde::__private::PhantomData,
                        },
                    )
                }
            }
        };
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl _serde::Serialize for MemType {
                fn serialize<__S>(
                    &self,
                    __serializer: __S,
                ) -> _serde::__private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    match *self {
                        MemType::RAM => _serde::Serializer::serialize_unit_variant(
                            __serializer,
                            "MemType",
                            0u32,
                            "RAM",
                        ),
                        MemType::ROM => _serde::Serializer::serialize_unit_variant(
                            __serializer,
                            "MemType",
                            1u32,
                            "ROM",
                        ),
                        MemType::Dev => _serde::Serializer::serialize_unit_variant(
                            __serializer,
                            "MemType",
                            2u32,
                            "Dev",
                        ),
                        MemType::Res => _serde::Serializer::serialize_unit_variant(
                            __serializer,
                            "MemType",
                            3u32,
                            "Res",
                        ),
                    }
                }
            }
        };
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for MemType {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match (&*self,) {
                    (&MemType::RAM,) => ::core::fmt::Formatter::write_str(f, "RAM"),
                    (&MemType::ROM,) => ::core::fmt::Formatter::write_str(f, "ROM"),
                    (&MemType::Dev,) => ::core::fmt::Formatter::write_str(f, "Dev"),
                    (&MemType::Res,) => ::core::fmt::Formatter::write_str(f, "Res"),
                }
            }
        }
        pub(crate) struct MemRegion {
            pub start: u64,
            pub end: u64,
            pub typ: MemType,
        }
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl<'de> _serde::Deserialize<'de> for MemRegion {
                fn deserialize<__D>(
                    __deserializer: __D,
                ) -> _serde::__private::Result<Self, __D::Error>
                where
                    __D: _serde::Deserializer<'de>,
                {
                    #[allow(non_camel_case_types)]
                    enum __Field {
                        __field0,
                        __field1,
                        __field2,
                        __ignore,
                    }
                    struct __FieldVisitor;
                    impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "field identifier")
                        }
                        fn visit_u64<__E>(
                            self,
                            __value: u64,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                0u64 => _serde::__private::Ok(__Field::__field0),
                                1u64 => _serde::__private::Ok(__Field::__field1),
                                2u64 => _serde::__private::Ok(__Field::__field2),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_str<__E>(
                            self,
                            __value: &str,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                "start" => _serde::__private::Ok(__Field::__field0),
                                "end" => _serde::__private::Ok(__Field::__field1),
                                "typ" => _serde::__private::Ok(__Field::__field2),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_bytes<__E>(
                            self,
                            __value: &[u8],
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                b"start" => _serde::__private::Ok(__Field::__field0),
                                b"end" => _serde::__private::Ok(__Field::__field1),
                                b"typ" => _serde::__private::Ok(__Field::__field2),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                    }
                    impl<'de> _serde::Deserialize<'de> for __Field {
                        #[inline]
                        fn deserialize<__D>(
                            __deserializer: __D,
                        ) -> _serde::__private::Result<Self, __D::Error>
                        where
                            __D: _serde::Deserializer<'de>,
                        {
                            _serde::Deserializer::deserialize_identifier(
                                __deserializer,
                                __FieldVisitor,
                            )
                        }
                    }
                    struct __Visitor<'de> {
                        marker: _serde::__private::PhantomData<MemRegion>,
                        lifetime: _serde::__private::PhantomData<&'de ()>,
                    }
                    impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                        type Value = MemRegion;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "struct MemRegion")
                        }
                        #[inline]
                        fn visit_seq<__A>(
                            self,
                            mut __seq: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::SeqAccess<'de>,
                        {
                            let __field0 = match match _serde::de::SeqAccess::next_element::<u64>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct MemRegion with 3 elements",
                                        ),
                                    );
                                }
                            };
                            let __field1 = match match _serde::de::SeqAccess::next_element::<u64>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            1usize,
                                            &"struct MemRegion with 3 elements",
                                        ),
                                    );
                                }
                            };
                            let __field2 = match match _serde::de::SeqAccess::next_element::<MemType>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            2usize,
                                            &"struct MemRegion with 3 elements",
                                        ),
                                    );
                                }
                            };
                            _serde::__private::Ok(MemRegion {
                                start: __field0,
                                end: __field1,
                                typ: __field2,
                            })
                        }
                        #[inline]
                        fn visit_map<__A>(
                            self,
                            mut __map: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::MapAccess<'de>,
                        {
                            let mut __field0: _serde::__private::Option<u64> =
                                _serde::__private::None;
                            let mut __field1: _serde::__private::Option<u64> =
                                _serde::__private::None;
                            let mut __field2: _serde::__private::Option<MemType> =
                                _serde::__private::None;
                            while let _serde::__private::Some(__key) =
                                match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            {
                                match __key {
                                    __Field::__field0 => {
                                        if _serde::__private::Option::is_some(&__field0) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "start",
                                                ),
                                            );
                                        }
                                        __field0 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<u64>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field1 => {
                                        if _serde::__private::Option::is_some(&__field1) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "end",
                                                ),
                                            );
                                        }
                                        __field1 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<u64>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field2 => {
                                        if _serde::__private::Option::is_some(&__field2) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "typ",
                                                ),
                                            );
                                        }
                                        __field2 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<MemType>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    _ => {
                                        let _ = match _serde::de::MapAccess::next_value::<
                                            _serde::de::IgnoredAny,
                                        >(
                                            &mut __map
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        };
                                    }
                                }
                            }
                            let __field0 = match __field0 {
                                _serde::__private::Some(__field0) => __field0,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("start") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field1 = match __field1 {
                                _serde::__private::Some(__field1) => __field1,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("end") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field2 = match __field2 {
                                _serde::__private::Some(__field2) => __field2,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("typ") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            _serde::__private::Ok(MemRegion {
                                start: __field0,
                                end: __field1,
                                typ: __field2,
                            })
                        }
                    }
                    const FIELDS: &'static [&'static str] = &["start", "end", "typ"];
                    _serde::Deserializer::deserialize_struct(
                        __deserializer,
                        "MemRegion",
                        FIELDS,
                        __Visitor {
                            marker: _serde::__private::PhantomData::<MemRegion>,
                            lifetime: _serde::__private::PhantomData,
                        },
                    )
                }
            }
        };
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl _serde::Serialize for MemRegion {
                fn serialize<__S>(
                    &self,
                    __serializer: __S,
                ) -> _serde::__private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    let mut __serde_state = match _serde::Serializer::serialize_struct(
                        __serializer,
                        "MemRegion",
                        false as usize + 1 + 1 + 1,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "start",
                        &self.start,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "end",
                        &self.end,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "typ",
                        &self.typ,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    _serde::ser::SerializeStruct::end(__serde_state)
                }
            }
        };
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for MemRegion {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    MemRegion {
                        start: ref __self_0_0,
                        end: ref __self_0_1,
                        typ: ref __self_0_2,
                    } => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_struct(f, "MemRegion");
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "start",
                            &&(*__self_0_0),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "end",
                            &&(*__self_0_1),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "typ",
                            &&(*__self_0_2),
                        );
                        ::core::fmt::DebugStruct::finish(debug_trait_builder)
                    }
                }
            }
        }
        pub(crate) struct PciId {
            pub vendor: u16,
            pub device: u16,
        }
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl<'de> _serde::Deserialize<'de> for PciId {
                fn deserialize<__D>(
                    __deserializer: __D,
                ) -> _serde::__private::Result<Self, __D::Error>
                where
                    __D: _serde::Deserializer<'de>,
                {
                    #[allow(non_camel_case_types)]
                    enum __Field {
                        __field0,
                        __field1,
                        __ignore,
                    }
                    struct __FieldVisitor;
                    impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "field identifier")
                        }
                        fn visit_u64<__E>(
                            self,
                            __value: u64,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                0u64 => _serde::__private::Ok(__Field::__field0),
                                1u64 => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_str<__E>(
                            self,
                            __value: &str,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                "vendor" => _serde::__private::Ok(__Field::__field0),
                                "device" => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_bytes<__E>(
                            self,
                            __value: &[u8],
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                b"vendor" => _serde::__private::Ok(__Field::__field0),
                                b"device" => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                    }
                    impl<'de> _serde::Deserialize<'de> for __Field {
                        #[inline]
                        fn deserialize<__D>(
                            __deserializer: __D,
                        ) -> _serde::__private::Result<Self, __D::Error>
                        where
                            __D: _serde::Deserializer<'de>,
                        {
                            _serde::Deserializer::deserialize_identifier(
                                __deserializer,
                                __FieldVisitor,
                            )
                        }
                    }
                    struct __Visitor<'de> {
                        marker: _serde::__private::PhantomData<PciId>,
                        lifetime: _serde::__private::PhantomData<&'de ()>,
                    }
                    impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                        type Value = PciId;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "struct PciId")
                        }
                        #[inline]
                        fn visit_seq<__A>(
                            self,
                            mut __seq: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::SeqAccess<'de>,
                        {
                            let __field0 = match match _serde::de::SeqAccess::next_element::<u16>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct PciId with 2 elements",
                                        ),
                                    );
                                }
                            };
                            let __field1 = match match _serde::de::SeqAccess::next_element::<u16>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            1usize,
                                            &"struct PciId with 2 elements",
                                        ),
                                    );
                                }
                            };
                            _serde::__private::Ok(PciId {
                                vendor: __field0,
                                device: __field1,
                            })
                        }
                        #[inline]
                        fn visit_map<__A>(
                            self,
                            mut __map: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::MapAccess<'de>,
                        {
                            let mut __field0: _serde::__private::Option<u16> =
                                _serde::__private::None;
                            let mut __field1: _serde::__private::Option<u16> =
                                _serde::__private::None;
                            while let _serde::__private::Some(__key) =
                                match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            {
                                match __key {
                                    __Field::__field0 => {
                                        if _serde::__private::Option::is_some(&__field0) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "vendor",
                                                ),
                                            );
                                        }
                                        __field0 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<u16>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field1 => {
                                        if _serde::__private::Option::is_some(&__field1) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "device",
                                                ),
                                            );
                                        }
                                        __field1 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<u16>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    _ => {
                                        let _ = match _serde::de::MapAccess::next_value::<
                                            _serde::de::IgnoredAny,
                                        >(
                                            &mut __map
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        };
                                    }
                                }
                            }
                            let __field0 = match __field0 {
                                _serde::__private::Some(__field0) => __field0,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("vendor") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field1 = match __field1 {
                                _serde::__private::Some(__field1) => __field1,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("device") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            _serde::__private::Ok(PciId {
                                vendor: __field0,
                                device: __field1,
                            })
                        }
                    }
                    const FIELDS: &'static [&'static str] = &["vendor", "device"];
                    _serde::Deserializer::deserialize_struct(
                        __deserializer,
                        "PciId",
                        FIELDS,
                        __Visitor {
                            marker: _serde::__private::PhantomData::<PciId>,
                            lifetime: _serde::__private::PhantomData,
                        },
                    )
                }
            }
        };
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl _serde::Serialize for PciId {
                fn serialize<__S>(
                    &self,
                    __serializer: __S,
                ) -> _serde::__private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    let mut __serde_state = match _serde::Serializer::serialize_struct(
                        __serializer,
                        "PciId",
                        false as usize + 1 + 1,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "vendor",
                        &self.vendor,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "device",
                        &self.device,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    _serde::ser::SerializeStruct::end(__serde_state)
                }
            }
        };
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for PciId {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    PciId {
                        vendor: ref __self_0_0,
                        device: ref __self_0_1,
                    } => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_struct(f, "PciId");
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "vendor",
                            &&(*__self_0_0),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "device",
                            &&(*__self_0_1),
                        );
                        ::core::fmt::DebugStruct::finish(debug_trait_builder)
                    }
                }
            }
        }
        pub(crate) struct PciBdf {
            pub bus: u16,
            pub device: u8,
            pub function: u8,
        }
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl<'de> _serde::Deserialize<'de> for PciBdf {
                fn deserialize<__D>(
                    __deserializer: __D,
                ) -> _serde::__private::Result<Self, __D::Error>
                where
                    __D: _serde::Deserializer<'de>,
                {
                    #[allow(non_camel_case_types)]
                    enum __Field {
                        __field0,
                        __field1,
                        __field2,
                        __ignore,
                    }
                    struct __FieldVisitor;
                    impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "field identifier")
                        }
                        fn visit_u64<__E>(
                            self,
                            __value: u64,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                0u64 => _serde::__private::Ok(__Field::__field0),
                                1u64 => _serde::__private::Ok(__Field::__field1),
                                2u64 => _serde::__private::Ok(__Field::__field2),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_str<__E>(
                            self,
                            __value: &str,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                "bus" => _serde::__private::Ok(__Field::__field0),
                                "device" => _serde::__private::Ok(__Field::__field1),
                                "function" => _serde::__private::Ok(__Field::__field2),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_bytes<__E>(
                            self,
                            __value: &[u8],
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                b"bus" => _serde::__private::Ok(__Field::__field0),
                                b"device" => _serde::__private::Ok(__Field::__field1),
                                b"function" => _serde::__private::Ok(__Field::__field2),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                    }
                    impl<'de> _serde::Deserialize<'de> for __Field {
                        #[inline]
                        fn deserialize<__D>(
                            __deserializer: __D,
                        ) -> _serde::__private::Result<Self, __D::Error>
                        where
                            __D: _serde::Deserializer<'de>,
                        {
                            _serde::Deserializer::deserialize_identifier(
                                __deserializer,
                                __FieldVisitor,
                            )
                        }
                    }
                    struct __Visitor<'de> {
                        marker: _serde::__private::PhantomData<PciBdf>,
                        lifetime: _serde::__private::PhantomData<&'de ()>,
                    }
                    impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                        type Value = PciBdf;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "struct PciBdf")
                        }
                        #[inline]
                        fn visit_seq<__A>(
                            self,
                            mut __seq: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::SeqAccess<'de>,
                        {
                            let __field0 = match match _serde::de::SeqAccess::next_element::<u16>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct PciBdf with 3 elements",
                                        ),
                                    );
                                }
                            };
                            let __field1 =
                                match match _serde::de::SeqAccess::next_element::<u8>(&mut __seq) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                } {
                                    _serde::__private::Some(__value) => __value,
                                    _serde::__private::None => {
                                        return _serde::__private::Err(
                                            _serde::de::Error::invalid_length(
                                                1usize,
                                                &"struct PciBdf with 3 elements",
                                            ),
                                        );
                                    }
                                };
                            let __field2 =
                                match match _serde::de::SeqAccess::next_element::<u8>(&mut __seq) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                } {
                                    _serde::__private::Some(__value) => __value,
                                    _serde::__private::None => {
                                        return _serde::__private::Err(
                                            _serde::de::Error::invalid_length(
                                                2usize,
                                                &"struct PciBdf with 3 elements",
                                            ),
                                        );
                                    }
                                };
                            _serde::__private::Ok(PciBdf {
                                bus: __field0,
                                device: __field1,
                                function: __field2,
                            })
                        }
                        #[inline]
                        fn visit_map<__A>(
                            self,
                            mut __map: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::MapAccess<'de>,
                        {
                            let mut __field0: _serde::__private::Option<u16> =
                                _serde::__private::None;
                            let mut __field1: _serde::__private::Option<u8> =
                                _serde::__private::None;
                            let mut __field2: _serde::__private::Option<u8> =
                                _serde::__private::None;
                            while let _serde::__private::Some(__key) =
                                match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            {
                                match __key {
                                    __Field::__field0 => {
                                        if _serde::__private::Option::is_some(&__field0) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "bus",
                                                ),
                                            );
                                        }
                                        __field0 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<u16>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field1 => {
                                        if _serde::__private::Option::is_some(&__field1) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "device",
                                                ),
                                            );
                                        }
                                        __field1 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<u8>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field2 => {
                                        if _serde::__private::Option::is_some(&__field2) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "function",
                                                ),
                                            );
                                        }
                                        __field2 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<u8>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    _ => {
                                        let _ = match _serde::de::MapAccess::next_value::<
                                            _serde::de::IgnoredAny,
                                        >(
                                            &mut __map
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        };
                                    }
                                }
                            }
                            let __field0 = match __field0 {
                                _serde::__private::Some(__field0) => __field0,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("bus") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field1 = match __field1 {
                                _serde::__private::Some(__field1) => __field1,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("device") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field2 = match __field2 {
                                _serde::__private::Some(__field2) => __field2,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("function") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            _serde::__private::Ok(PciBdf {
                                bus: __field0,
                                device: __field1,
                                function: __field2,
                            })
                        }
                    }
                    const FIELDS: &'static [&'static str] = &["bus", "device", "function"];
                    _serde::Deserializer::deserialize_struct(
                        __deserializer,
                        "PciBdf",
                        FIELDS,
                        __Visitor {
                            marker: _serde::__private::PhantomData::<PciBdf>,
                            lifetime: _serde::__private::PhantomData,
                        },
                    )
                }
            }
        };
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl _serde::Serialize for PciBdf {
                fn serialize<__S>(
                    &self,
                    __serializer: __S,
                ) -> _serde::__private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    let mut __serde_state = match _serde::Serializer::serialize_struct(
                        __serializer,
                        "PciBdf",
                        false as usize + 1 + 1 + 1,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "bus",
                        &self.bus,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "device",
                        &self.device,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "function",
                        &self.function,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    _serde::ser::SerializeStruct::end(__serde_state)
                }
            }
        };
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for PciBdf {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    PciBdf {
                        bus: ref __self_0_0,
                        device: ref __self_0_1,
                        function: ref __self_0_2,
                    } => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_struct(f, "PciBdf");
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "bus",
                            &&(*__self_0_0),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "device",
                            &&(*__self_0_1),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "function",
                            &&(*__self_0_2),
                        );
                        ::core::fmt::DebugStruct::finish(debug_trait_builder)
                    }
                }
            }
        }
        pub(crate) struct DevPorts {
            pub device: u32,
            pub ports: Vec<u16>,
        }
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl<'de> _serde::Deserialize<'de> for DevPorts {
                fn deserialize<__D>(
                    __deserializer: __D,
                ) -> _serde::__private::Result<Self, __D::Error>
                where
                    __D: _serde::Deserializer<'de>,
                {
                    #[allow(non_camel_case_types)]
                    enum __Field {
                        __field0,
                        __field1,
                        __ignore,
                    }
                    struct __FieldVisitor;
                    impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "field identifier")
                        }
                        fn visit_u64<__E>(
                            self,
                            __value: u64,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                0u64 => _serde::__private::Ok(__Field::__field0),
                                1u64 => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_str<__E>(
                            self,
                            __value: &str,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                "device" => _serde::__private::Ok(__Field::__field0),
                                "ports" => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_bytes<__E>(
                            self,
                            __value: &[u8],
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                b"device" => _serde::__private::Ok(__Field::__field0),
                                b"ports" => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                    }
                    impl<'de> _serde::Deserialize<'de> for __Field {
                        #[inline]
                        fn deserialize<__D>(
                            __deserializer: __D,
                        ) -> _serde::__private::Result<Self, __D::Error>
                        where
                            __D: _serde::Deserializer<'de>,
                        {
                            _serde::Deserializer::deserialize_identifier(
                                __deserializer,
                                __FieldVisitor,
                            )
                        }
                    }
                    struct __Visitor<'de> {
                        marker: _serde::__private::PhantomData<DevPorts>,
                        lifetime: _serde::__private::PhantomData<&'de ()>,
                    }
                    impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                        type Value = DevPorts;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "struct DevPorts")
                        }
                        #[inline]
                        fn visit_seq<__A>(
                            self,
                            mut __seq: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::SeqAccess<'de>,
                        {
                            let __field0 = match match _serde::de::SeqAccess::next_element::<u32>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct DevPorts with 2 elements",
                                        ),
                                    );
                                }
                            };
                            let __field1 = match match _serde::de::SeqAccess::next_element::<Vec<u16>>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            1usize,
                                            &"struct DevPorts with 2 elements",
                                        ),
                                    );
                                }
                            };
                            _serde::__private::Ok(DevPorts {
                                device: __field0,
                                ports: __field1,
                            })
                        }
                        #[inline]
                        fn visit_map<__A>(
                            self,
                            mut __map: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::MapAccess<'de>,
                        {
                            let mut __field0: _serde::__private::Option<u32> =
                                _serde::__private::None;
                            let mut __field1: _serde::__private::Option<Vec<u16>> =
                                _serde::__private::None;
                            while let _serde::__private::Some(__key) =
                                match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            {
                                match __key {
                                    __Field::__field0 => {
                                        if _serde::__private::Option::is_some(&__field0) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "device",
                                                ),
                                            );
                                        }
                                        __field0 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<u32>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field1 => {
                                        if _serde::__private::Option::is_some(&__field1) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "ports",
                                                ),
                                            );
                                        }
                                        __field1 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<Vec<u16>>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    _ => {
                                        let _ = match _serde::de::MapAccess::next_value::<
                                            _serde::de::IgnoredAny,
                                        >(
                                            &mut __map
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        };
                                    }
                                }
                            }
                            let __field0 = match __field0 {
                                _serde::__private::Some(__field0) => __field0,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("device") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field1 = match __field1 {
                                _serde::__private::Some(__field1) => __field1,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("ports") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            _serde::__private::Ok(DevPorts {
                                device: __field0,
                                ports: __field1,
                            })
                        }
                    }
                    const FIELDS: &'static [&'static str] = &["device", "ports"];
                    _serde::Deserializer::deserialize_struct(
                        __deserializer,
                        "DevPorts",
                        FIELDS,
                        __Visitor {
                            marker: _serde::__private::PhantomData::<DevPorts>,
                            lifetime: _serde::__private::PhantomData,
                        },
                    )
                }
            }
        };
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl _serde::Serialize for DevPorts {
                fn serialize<__S>(
                    &self,
                    __serializer: __S,
                ) -> _serde::__private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    let mut __serde_state = match _serde::Serializer::serialize_struct(
                        __serializer,
                        "DevPorts",
                        false as usize + 1 + 1,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "device",
                        &self.device,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "ports",
                        &self.ports,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    _serde::ser::SerializeStruct::end(__serde_state)
                }
            }
        };
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for DevPorts {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    DevPorts {
                        device: ref __self_0_0,
                        ports: ref __self_0_1,
                    } => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_struct(f, "DevPorts");
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "device",
                            &&(*__self_0_0),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "ports",
                            &&(*__self_0_1),
                        );
                        ::core::fmt::DebugStruct::finish(debug_trait_builder)
                    }
                }
            }
        }
        pub(crate) struct VmDescr {
            pub vcpus: Vec<u32>,
            pub ioapics: Vec<u32>,
            pub mem: Vec<MemRegion>,
            pub pci: Vec<(PciId, PciBdf)>,
            pub ports: Vec<DevPorts>,
        }
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl<'de> _serde::Deserialize<'de> for VmDescr {
                fn deserialize<__D>(
                    __deserializer: __D,
                ) -> _serde::__private::Result<Self, __D::Error>
                where
                    __D: _serde::Deserializer<'de>,
                {
                    #[allow(non_camel_case_types)]
                    enum __Field {
                        __field0,
                        __field1,
                        __field2,
                        __field3,
                        __field4,
                        __ignore,
                    }
                    struct __FieldVisitor;
                    impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "field identifier")
                        }
                        fn visit_u64<__E>(
                            self,
                            __value: u64,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                0u64 => _serde::__private::Ok(__Field::__field0),
                                1u64 => _serde::__private::Ok(__Field::__field1),
                                2u64 => _serde::__private::Ok(__Field::__field2),
                                3u64 => _serde::__private::Ok(__Field::__field3),
                                4u64 => _serde::__private::Ok(__Field::__field4),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_str<__E>(
                            self,
                            __value: &str,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                "vcpus" => _serde::__private::Ok(__Field::__field0),
                                "ioapics" => _serde::__private::Ok(__Field::__field1),
                                "mem" => _serde::__private::Ok(__Field::__field2),
                                "pci" => _serde::__private::Ok(__Field::__field3),
                                "ports" => _serde::__private::Ok(__Field::__field4),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_bytes<__E>(
                            self,
                            __value: &[u8],
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                b"vcpus" => _serde::__private::Ok(__Field::__field0),
                                b"ioapics" => _serde::__private::Ok(__Field::__field1),
                                b"mem" => _serde::__private::Ok(__Field::__field2),
                                b"pci" => _serde::__private::Ok(__Field::__field3),
                                b"ports" => _serde::__private::Ok(__Field::__field4),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                    }
                    impl<'de> _serde::Deserialize<'de> for __Field {
                        #[inline]
                        fn deserialize<__D>(
                            __deserializer: __D,
                        ) -> _serde::__private::Result<Self, __D::Error>
                        where
                            __D: _serde::Deserializer<'de>,
                        {
                            _serde::Deserializer::deserialize_identifier(
                                __deserializer,
                                __FieldVisitor,
                            )
                        }
                    }
                    struct __Visitor<'de> {
                        marker: _serde::__private::PhantomData<VmDescr>,
                        lifetime: _serde::__private::PhantomData<&'de ()>,
                    }
                    impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                        type Value = VmDescr;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "struct VmDescr")
                        }
                        #[inline]
                        fn visit_seq<__A>(
                            self,
                            mut __seq: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::SeqAccess<'de>,
                        {
                            let __field0 = match match _serde::de::SeqAccess::next_element::<Vec<u32>>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct VmDescr with 5 elements",
                                        ),
                                    );
                                }
                            };
                            let __field1 = match match _serde::de::SeqAccess::next_element::<Vec<u32>>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            1usize,
                                            &"struct VmDescr with 5 elements",
                                        ),
                                    );
                                }
                            };
                            let __field2 = match match _serde::de::SeqAccess::next_element::<
                                Vec<MemRegion>,
                            >(&mut __seq)
                            {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            2usize,
                                            &"struct VmDescr with 5 elements",
                                        ),
                                    );
                                }
                            };
                            let __field3 = match match _serde::de::SeqAccess::next_element::<
                                Vec<(PciId, PciBdf)>,
                            >(&mut __seq)
                            {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            3usize,
                                            &"struct VmDescr with 5 elements",
                                        ),
                                    );
                                }
                            };
                            let __field4 = match match _serde::de::SeqAccess::next_element::<
                                Vec<DevPorts>,
                            >(&mut __seq)
                            {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            4usize,
                                            &"struct VmDescr with 5 elements",
                                        ),
                                    );
                                }
                            };
                            _serde::__private::Ok(VmDescr {
                                vcpus: __field0,
                                ioapics: __field1,
                                mem: __field2,
                                pci: __field3,
                                ports: __field4,
                            })
                        }
                        #[inline]
                        fn visit_map<__A>(
                            self,
                            mut __map: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::MapAccess<'de>,
                        {
                            let mut __field0: _serde::__private::Option<Vec<u32>> =
                                _serde::__private::None;
                            let mut __field1: _serde::__private::Option<Vec<u32>> =
                                _serde::__private::None;
                            let mut __field2: _serde::__private::Option<Vec<MemRegion>> =
                                _serde::__private::None;
                            let mut __field3: _serde::__private::Option<Vec<(PciId, PciBdf)>> =
                                _serde::__private::None;
                            let mut __field4: _serde::__private::Option<Vec<DevPorts>> =
                                _serde::__private::None;
                            while let _serde::__private::Some(__key) =
                                match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            {
                                match __key {
                                    __Field::__field0 => {
                                        if _serde::__private::Option::is_some(&__field0) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "vcpus",
                                                ),
                                            );
                                        }
                                        __field0 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<Vec<u32>>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field1 => {
                                        if _serde::__private::Option::is_some(&__field1) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "ioapics",
                                                ),
                                            );
                                        }
                                        __field1 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<Vec<u32>>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field2 => {
                                        if _serde::__private::Option::is_some(&__field2) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "mem",
                                                ),
                                            );
                                        }
                                        __field2 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<Vec<MemRegion>>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field3 => {
                                        if _serde::__private::Option::is_some(&__field3) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "pci",
                                                ),
                                            );
                                        }
                                        __field3 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<
                                                Vec<(PciId, PciBdf)>,
                                            >(
                                                &mut __map
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field4 => {
                                        if _serde::__private::Option::is_some(&__field4) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "ports",
                                                ),
                                            );
                                        }
                                        __field4 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<Vec<DevPorts>>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    _ => {
                                        let _ = match _serde::de::MapAccess::next_value::<
                                            _serde::de::IgnoredAny,
                                        >(
                                            &mut __map
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        };
                                    }
                                }
                            }
                            let __field0 = match __field0 {
                                _serde::__private::Some(__field0) => __field0,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("vcpus") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field1 = match __field1 {
                                _serde::__private::Some(__field1) => __field1,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("ioapics") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field2 = match __field2 {
                                _serde::__private::Some(__field2) => __field2,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("mem") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field3 = match __field3 {
                                _serde::__private::Some(__field3) => __field3,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("pci") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field4 = match __field4 {
                                _serde::__private::Some(__field4) => __field4,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("ports") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            _serde::__private::Ok(VmDescr {
                                vcpus: __field0,
                                ioapics: __field1,
                                mem: __field2,
                                pci: __field3,
                                ports: __field4,
                            })
                        }
                    }
                    const FIELDS: &'static [&'static str] =
                        &["vcpus", "ioapics", "mem", "pci", "ports"];
                    _serde::Deserializer::deserialize_struct(
                        __deserializer,
                        "VmDescr",
                        FIELDS,
                        __Visitor {
                            marker: _serde::__private::PhantomData::<VmDescr>,
                            lifetime: _serde::__private::PhantomData,
                        },
                    )
                }
            }
        };
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl _serde::Serialize for VmDescr {
                fn serialize<__S>(
                    &self,
                    __serializer: __S,
                ) -> _serde::__private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    let mut __serde_state = match _serde::Serializer::serialize_struct(
                        __serializer,
                        "VmDescr",
                        false as usize + 1 + 1 + 1 + 1 + 1,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "vcpus",
                        &self.vcpus,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "ioapics",
                        &self.ioapics,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "mem",
                        &self.mem,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "pci",
                        &self.pci,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "ports",
                        &self.ports,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    _serde::ser::SerializeStruct::end(__serde_state)
                }
            }
        };
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for VmDescr {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    VmDescr {
                        vcpus: ref __self_0_0,
                        ioapics: ref __self_0_1,
                        mem: ref __self_0_2,
                        pci: ref __self_0_3,
                        ports: ref __self_0_4,
                    } => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_struct(f, "VmDescr");
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "vcpus",
                            &&(*__self_0_0),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "ioapics",
                            &&(*__self_0_1),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "mem",
                            &&(*__self_0_2),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "pci",
                            &&(*__self_0_3),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "ports",
                            &&(*__self_0_4),
                        );
                        ::core::fmt::DebugStruct::finish(debug_trait_builder)
                    }
                }
            }
        }
        impl VmDescr {
            pub fn new(_instance: &Instance) -> VmDescr {
                let vcpus = <[_]>::into_vec(box [0, 1, 2, 3]);
                VmDescr {
                    vcpus,
                    ioapics: Vec::new(),
                    mem: Vec::new(),
                    pci: Vec::new(),
                    ports: Vec::new(),
                }
            }
        }
        pub(crate) struct Preamble {
            pub vm_descr: VmDescr,
            pub blobs: Vec<Vec<u8>>,
        }
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl<'de> _serde::Deserialize<'de> for Preamble {
                fn deserialize<__D>(
                    __deserializer: __D,
                ) -> _serde::__private::Result<Self, __D::Error>
                where
                    __D: _serde::Deserializer<'de>,
                {
                    #[allow(non_camel_case_types)]
                    enum __Field {
                        __field0,
                        __field1,
                        __ignore,
                    }
                    struct __FieldVisitor;
                    impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                        type Value = __Field;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "field identifier")
                        }
                        fn visit_u64<__E>(
                            self,
                            __value: u64,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                0u64 => _serde::__private::Ok(__Field::__field0),
                                1u64 => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_str<__E>(
                            self,
                            __value: &str,
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                "vm_descr" => _serde::__private::Ok(__Field::__field0),
                                "blobs" => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                        fn visit_bytes<__E>(
                            self,
                            __value: &[u8],
                        ) -> _serde::__private::Result<Self::Value, __E>
                        where
                            __E: _serde::de::Error,
                        {
                            match __value {
                                b"vm_descr" => _serde::__private::Ok(__Field::__field0),
                                b"blobs" => _serde::__private::Ok(__Field::__field1),
                                _ => _serde::__private::Ok(__Field::__ignore),
                            }
                        }
                    }
                    impl<'de> _serde::Deserialize<'de> for __Field {
                        #[inline]
                        fn deserialize<__D>(
                            __deserializer: __D,
                        ) -> _serde::__private::Result<Self, __D::Error>
                        where
                            __D: _serde::Deserializer<'de>,
                        {
                            _serde::Deserializer::deserialize_identifier(
                                __deserializer,
                                __FieldVisitor,
                            )
                        }
                    }
                    struct __Visitor<'de> {
                        marker: _serde::__private::PhantomData<Preamble>,
                        lifetime: _serde::__private::PhantomData<&'de ()>,
                    }
                    impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                        type Value = Preamble;
                        fn expecting(
                            &self,
                            __formatter: &mut _serde::__private::Formatter,
                        ) -> _serde::__private::fmt::Result {
                            _serde::__private::Formatter::write_str(__formatter, "struct Preamble")
                        }
                        #[inline]
                        fn visit_seq<__A>(
                            self,
                            mut __seq: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::SeqAccess<'de>,
                        {
                            let __field0 = match match _serde::de::SeqAccess::next_element::<VmDescr>(
                                &mut __seq,
                            ) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct Preamble with 2 elements",
                                        ),
                                    );
                                }
                            };
                            let __field1 = match match _serde::de::SeqAccess::next_element::<
                                Vec<Vec<u8>>,
                            >(&mut __seq)
                            {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            1usize,
                                            &"struct Preamble with 2 elements",
                                        ),
                                    );
                                }
                            };
                            _serde::__private::Ok(Preamble {
                                vm_descr: __field0,
                                blobs: __field1,
                            })
                        }
                        #[inline]
                        fn visit_map<__A>(
                            self,
                            mut __map: __A,
                        ) -> _serde::__private::Result<Self::Value, __A::Error>
                        where
                            __A: _serde::de::MapAccess<'de>,
                        {
                            let mut __field0: _serde::__private::Option<VmDescr> =
                                _serde::__private::None;
                            let mut __field1: _serde::__private::Option<Vec<Vec<u8>>> =
                                _serde::__private::None;
                            while let _serde::__private::Some(__key) =
                                match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            {
                                match __key {
                                    __Field::__field0 => {
                                        if _serde::__private::Option::is_some(&__field0) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "vm_descr",
                                                ),
                                            );
                                        }
                                        __field0 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<VmDescr>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    __Field::__field1 => {
                                        if _serde::__private::Option::is_some(&__field1) {
                                            return _serde::__private::Err(
                                                <__A::Error as _serde::de::Error>::duplicate_field(
                                                    "blobs",
                                                ),
                                            );
                                        }
                                        __field1 = _serde::__private::Some(
                                            match _serde::de::MapAccess::next_value::<Vec<Vec<u8>>>(
                                                &mut __map,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            },
                                        );
                                    }
                                    _ => {
                                        let _ = match _serde::de::MapAccess::next_value::<
                                            _serde::de::IgnoredAny,
                                        >(
                                            &mut __map
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        };
                                    }
                                }
                            }
                            let __field0 = match __field0 {
                                _serde::__private::Some(__field0) => __field0,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("vm_descr") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            let __field1 = match __field1 {
                                _serde::__private::Some(__field1) => __field1,
                                _serde::__private::None => {
                                    match _serde::__private::de::missing_field("blobs") {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    }
                                }
                            };
                            _serde::__private::Ok(Preamble {
                                vm_descr: __field0,
                                blobs: __field1,
                            })
                        }
                    }
                    const FIELDS: &'static [&'static str] = &["vm_descr", "blobs"];
                    _serde::Deserializer::deserialize_struct(
                        __deserializer,
                        "Preamble",
                        FIELDS,
                        __Visitor {
                            marker: _serde::__private::PhantomData::<Preamble>,
                            lifetime: _serde::__private::PhantomData,
                        },
                    )
                }
            }
        };
        #[doc(hidden)]
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const _: () = {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
            #[automatically_derived]
            impl _serde::Serialize for Preamble {
                fn serialize<__S>(
                    &self,
                    __serializer: __S,
                ) -> _serde::__private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    let mut __serde_state = match _serde::Serializer::serialize_struct(
                        __serializer,
                        "Preamble",
                        false as usize + 1 + 1,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "vm_descr",
                        &self.vm_descr,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    match _serde::ser::SerializeStruct::serialize_field(
                        &mut __serde_state,
                        "blobs",
                        &self.blobs,
                    ) {
                        _serde::__private::Ok(__val) => __val,
                        _serde::__private::Err(__err) => {
                            return _serde::__private::Err(__err);
                        }
                    };
                    _serde::ser::SerializeStruct::end(__serde_state)
                }
            }
        };
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl ::core::fmt::Debug for Preamble {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    Preamble {
                        vm_descr: ref __self_0_0,
                        blobs: ref __self_0_1,
                    } => {
                        let debug_trait_builder =
                            &mut ::core::fmt::Formatter::debug_struct(f, "Preamble");
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "vm_descr",
                            &&(*__self_0_0),
                        );
                        let _ = ::core::fmt::DebugStruct::field(
                            debug_trait_builder,
                            "blobs",
                            &&(*__self_0_1),
                        );
                        ::core::fmt::DebugStruct::finish(debug_trait_builder)
                    }
                }
            }
        }
        impl Preamble {
            pub fn new(instance: &Instance) -> Preamble {
                Preamble {
                    vm_descr: VmDescr::new(instance),
                    blobs: Vec::new(),
                }
            }
        }
    }
    mod source {
        use futures::{future, SinkExt, StreamExt};
        use hyper::upgrade::Upgraded;
        use propolis::common::GuestAddr;
        use propolis::instance::{MigrateRole, ReqState};
        use propolis::inventory::Order;
        use propolis::migrate::{MigrateStateError, Migrator};
        use slog::{error, info};
        use std::io;
        use std::ops::Range;
        use std::sync::Arc;
        use std::time::Duration;
        use tokio::{task, time};
        use tokio_util::codec::Framed;
        use crate::migrate::codec::{self, LiveMigrationFramer};
        use crate::migrate::memx;
        use crate::migrate::preamble::Preamble;
        use crate::migrate::{Device, MigrateContext, MigrateError, MigrationState, PageIter};
        pub async fn migrate(
            mctx: Arc<MigrateContext>,
            conn: Upgraded,
        ) -> Result<(), MigrateError> {
            let mut proto = SourceProtocol::new(mctx, conn);
            if let Err(err) = proto.run().await {
                proto.mctx.set_state(MigrationState::Error).await;
                let _ = proto.conn.send(codec::Message::Error(err.clone())).await;
                return Err(err);
            }
            Ok(())
        }
        struct SourceProtocol {
            /// The migration context which also contains the Instance handle.
            mctx: Arc<MigrateContext>,
            /// Transport to the destination Instance.
            conn: Framed<Upgraded, LiveMigrationFramer>,
        }
        impl SourceProtocol {
            fn new(mctx: Arc<MigrateContext>, conn: Upgraded) -> Self {
                let codec_log = mctx.log.new(::slog::OwnedKV(()));
                Self {
                    mctx,
                    conn: Framed::new(conn, LiveMigrationFramer::new(codec_log)),
                }
            }
            fn log(&self) -> &slog::Logger {
                &self.mctx.log
            }
            async fn run(&mut self) -> Result<(), MigrateError> {
                self.start();
                self.sync().await?;
                self.ram_push().await?;
                self.pause().await?;
                self.device_state().await?;
                self.arch_state().await?;
                self.ram_pull().await?;
                self.finish().await?;
                self.end()?;
                Ok(())
            }
            fn start(&mut self) {
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 76u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["Entering Source Migration Task"],
                                &match () {
                                    _args => [],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
            }
            async fn sync(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Sync).await;
                let preamble = Preamble::new(self.mctx.instance.as_ref());
                let s = ron::ser::to_string(&preamble).map_err(codec::ProtocolError::from)?;
                self.send_msg(codec::Message::Serialized(s)).await?;
                self.read_ok().await
            }
            async fn ram_push(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::RamPush).await;
                let vmm_ram_range = self.vmm_ram_bounds().await?;
                let req_ram_range = self.read_mem_query().await?;
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 92u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["ram_push: got query for range ", ", vm range "],
                                &match (&req_ram_range, &vmm_ram_range) {
                                    _args => [
                                        ::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Debug::fmt,
                                        ),
                                        ::core::fmt::ArgumentV1::new(
                                            _args.1,
                                            ::core::fmt::Debug::fmt,
                                        ),
                                    ],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                self.offer_ram(vmm_ram_range, req_ram_range).await?;
                loop {
                    let m = self.read_msg().await?;
                    if ::slog::Level::Info.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log(), &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/source.rs",
                                    line: 102u32,
                                    column: 13u32,
                                    function: "",
                                    module: "propolis_server::migrate::source",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Info,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["ram_push: source xfer phase recvd "],
                                    &match (&m,) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Debug::fmt,
                                        )],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    match m {
                        codec::Message::MemDone => break,
                        codec::Message::MemFetch(start, end, bits) => {
                            if !memx::validate_bitmap(start, end, &bits) {
                                if ::slog::Level::Error.as_usize()
                                    <= ::slog::__slog_static_max_level().as_usize()
                                {
                                    ::slog::Logger::log(&self.log(), &{
                                        #[allow(dead_code)]
                                        static RS: ::slog::RecordStatic<'static> = {
                                            static LOC: ::slog::RecordLocation =
                                                ::slog::RecordLocation {
                                                    file: "server/src/lib/migrate/source.rs",
                                                    line: 107u32,
                                                    column: 25u32,
                                                    function: "",
                                                    module: "propolis_server::migrate::source",
                                                };
                                            ::slog::RecordStatic {
                                                location: &LOC,
                                                level: ::slog::Level::Error,
                                                tag: "",
                                            }
                                        };
                                        ::slog::Record::new(
                                            &RS,
                                            &::core::fmt::Arguments::new_v1(
                                                &["invalid bitmap"],
                                                &match () {
                                                    _args => [],
                                                },
                                            ),
                                            ::slog::BorrowedKV(&()),
                                        )
                                    })
                                };
                                return Err(MigrateError::Phase);
                            }
                            self.xfer_ram(start, end, &bits).await?;
                        }
                        _ => return Err(MigrateError::UnexpectedMessage),
                    };
                }
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 120u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["ram_push: done sending ram"],
                                &match () {
                                    _args => [],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                self.mctx.set_state(MigrationState::Pause).await;
                Ok(())
            }
            async fn offer_ram(
                &mut self,
                vmm_ram_range: Range<GuestAddr>,
                req_ram_range: Range<u64>,
            ) -> Result<(), MigrateError> {
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 130u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["offering ram"],
                                &match () {
                                    _args => [],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                let vmm_ram_start = vmm_ram_range.start;
                let vmm_ram_end = vmm_ram_range.end;
                let mut bits = [0u8; 4096];
                let req_start_gpa = req_ram_range.start;
                let req_end_gpa = req_ram_range.end;
                let start_gpa = req_start_gpa.max(vmm_ram_start.0);
                let end_gpa = req_end_gpa.min(vmm_ram_end.0);
                let step = bits.len() * 8 * 4096;
                for gpa in (start_gpa..end_gpa).step_by(step) {
                    self.track_dirty(GuestAddr(0), &mut bits).await?;
                    if bits.iter().all(|&b| b == 0) {
                        continue;
                    }
                    let end = end_gpa.min(gpa + step as u64);
                    self.send_msg(memx::make_mem_offer(gpa, end, &bits)).await?;
                }
                self.send_msg(codec::Message::MemEnd(req_start_gpa, req_end_gpa))
                    .await
            }
            async fn xfer_ram(
                &mut self,
                start: u64,
                end: u64,
                bits: &[u8],
            ) -> Result<(), MigrateError> {
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 156u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["ram_push: xfer RAM between ", " and "],
                                &match (&start, &end) {
                                    _args => [
                                        ::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Display::fmt,
                                        ),
                                        ::core::fmt::ArgumentV1::new(
                                            _args.1,
                                            ::core::fmt::Display::fmt,
                                        ),
                                    ],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                self.send_msg(memx::make_mem_xfer(start, end, bits)).await?;
                for addr in PageIter::new(start, end, bits) {
                    let mut bytes = [0u8; 4096];
                    self.read_guest_mem(GuestAddr(addr), &mut bytes).await?;
                    self.send_msg(codec::Message::Page(bytes.into())).await?;
                }
                Ok(())
            }
            async fn pause(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Pause).await;
                let mut devices = ::alloc::vec::Vec::new();
                let _ = self
                    .mctx
                    .instance
                    .inv()
                    .for_each_node(Order::Post, |_, rec| {
                        devices.push((rec.name().to_owned(), Arc::clone(rec.entity())));
                        Ok::<_, ()>(())
                    });
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 179u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["Pausing devices"],
                                &match () {
                                    _args => [],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                let (pause_tx, pause_rx) = std::sync::mpsc::channel();
                self.mctx
                    .instance
                    .migrate_pause(self.mctx.async_ctx.context_id(), pause_rx)?;
                let mut migrate_ready_futs = ::alloc::vec::Vec::new();
                for (name, device) in &devices {
                    let log = self.log().new(::slog::OwnedKV((
                        ::slog::SingleKV::from(("device", name.clone())),
                        (),
                    )));
                    let device = Arc::clone(device);
                    let pause_fut = device.paused();
                    migrate_ready_futs.push(task::spawn(async move {
                        if let Err(_) = time::timeout(Duration::from_secs(2), pause_fut).await {
                            if ::slog::Level::Error.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&log, &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/migrate/source.rs",
                                                line: 195u32,
                                                column: 21u32,
                                                function: "",
                                                module: "propolis_server::migrate::source",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Error,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["Timed out pausing device"],
                                            &match () {
                                                _args => [],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                            return Err(device);
                        }
                        if ::slog::Level::Info.as_usize()
                            <= ::slog::__slog_static_max_level().as_usize()
                        {
                            ::slog::Logger::log(&log, &{
                                #[allow(dead_code)]
                                static RS: ::slog::RecordStatic<'static> = {
                                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                        file: "server/src/lib/migrate/source.rs",
                                        line: 198u32,
                                        column: 17u32,
                                        function: "",
                                        module: "propolis_server::migrate::source",
                                    };
                                    ::slog::RecordStatic {
                                        location: &LOC,
                                        level: ::slog::Level::Info,
                                        tag: "",
                                    }
                                };
                                ::slog::Record::new(
                                    &RS,
                                    &::core::fmt::Arguments::new_v1(
                                        &["Paused device"],
                                        &match () {
                                            _args => [],
                                        },
                                    ),
                                    ::slog::BorrowedKV(&()),
                                )
                            })
                        };
                        Ok(())
                    }));
                }
                let pause = future::join_all(migrate_ready_futs)
                    .await
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>();
                let timed_out = match pause {
                    Ok(future_res) => future_res
                        .into_iter()
                        .filter(Result::is_err)
                        .map(Result::unwrap_err)
                        .collect::<Vec<_>>(),
                    Err(err) => {
                        if ::slog::Level::Error.as_usize()
                            <= ::slog::__slog_static_max_level().as_usize()
                        {
                            ::slog::Logger::log(&self.log(), &{
                                #[allow(dead_code)]
                                static RS: ::slog::RecordStatic<'static> = {
                                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                        file: "server/src/lib/migrate/source.rs",
                                        line: 219u32,
                                        column: 17u32,
                                        function: "",
                                        module: "propolis_server::migrate::source",
                                    };
                                    ::slog::RecordStatic {
                                        location: &LOC,
                                        level: ::slog::Level::Error,
                                        tag: "",
                                    }
                                };
                                ::slog::Record::new(
                                    &RS,
                                    &::core::fmt::Arguments::new_v1(
                                        &["joining paused devices future failed: "],
                                        &match (&err,) {
                                            _args => [::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Display::fmt,
                                            )],
                                        },
                                    ),
                                    ::slog::BorrowedKV(&()),
                                )
                            })
                        };
                        return Err(MigrateError::SourcePause);
                    }
                };
                if !timed_out.is_empty() {
                    if ::slog::Level::Error.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&self.log(), &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/source.rs",
                                    line: 230u32,
                                    column: 13u32,
                                    function: "",
                                    module: "propolis_server::migrate::source",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Error,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["Failed to pause all devices: "],
                                    &match (&timed_out,) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Debug::fmt,
                                        )],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    return Err(MigrateError::SourcePause);
                }
                pause_tx.send(()).unwrap();
                Ok(())
            }
            async fn device_state(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Device).await;
                let dispctx = self
                    .mctx
                    .async_ctx
                    .dispctx()
                    .await
                    .ok_or_else(|| MigrateError::InstanceNotInitialized)?;
                let mut device_states = ::alloc::vec::Vec::new();
                self . mctx . instance . inv () . for_each_node (Order :: Pre , | _ , rec | { let entity = rec . entity () ; match entity . migrate () { Migrator :: NonMigratable => { if :: slog :: Level :: Error . as_usize () <= :: slog :: __slog_static_max_level () . as_usize () { :: slog :: Logger :: log (& self . log () , & { # [allow (dead_code)] static RS : :: slog :: RecordStatic < 'static > = { static LOC : :: slog :: RecordLocation = :: slog :: RecordLocation { file : "server/src/lib/migrate/source.rs" , line : 256u32 , column : 21u32 , function : "" , module : "propolis_server::migrate::source" , } ; :: slog :: RecordStatic { location : & LOC , level : :: slog :: Level :: Error , tag : "" , } } ; :: slog :: Record :: new (& RS , & :: core :: fmt :: Arguments :: new_v1 (& ["Can\'t migrate instance with non-migratable device (" , ")"] , & match (& rec . name () ,) { _args => [:: core :: fmt :: ArgumentV1 :: new (_args . 0 , :: core :: fmt :: Display :: fmt)] , }) , :: slog :: BorrowedKV (& ())) }) } ; return Err (MigrateError :: DeviceState (MigrateStateError :: NonMigratable)) ; } Migrator :: Simple => { } Migrator :: Custom (migrate) => { let payload = migrate . export (& dispctx) ; device_states . push (Device { instance_name : rec . name () . to_owned () , payload : ron :: ser :: to_string (& payload) . map_err (codec :: ProtocolError :: from) ? , }) ; } } Ok (()) }) ? ;
                drop(dispctx);
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 274u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1_formatted(
                                &["Device States: "],
                                &match (&device_states,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Debug::fmt,
                                    )],
                                },
                                &[::core::fmt::rt::v1::Argument {
                                    position: 0usize,
                                    format: ::core::fmt::rt::v1::FormatSpec {
                                        fill: ' ',
                                        align: ::core::fmt::rt::v1::Alignment::Unknown,
                                        flags: 4u32,
                                        precision: ::core::fmt::rt::v1::Count::Implied,
                                        width: ::core::fmt::rt::v1::Count::Implied,
                                    },
                                }],
                                unsafe { ::core::fmt::UnsafeArg::new() },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                self.send_msg(codec::Message::Serialized(
                    ron::ser::to_string(&device_states).map_err(codec::ProtocolError::from)?,
                ))
                .await?;
                self.send_msg(codec::Message::Okay).await?;
                self.read_ok().await
            }
            async fn arch_state(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Arch).await;
                self.read_ok().await?;
                self.send_msg(codec::Message::Okay).await
            }
            async fn ram_pull(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::RamPush).await;
                let m = self.read_msg().await?;
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 295u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["ram_pull: got query "],
                                &match (&m,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Debug::fmt,
                                    )],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                self.mctx.set_state(MigrationState::Pause).await;
                self.mctx.set_state(MigrationState::RamPushDirty).await;
                self.send_msg(codec::Message::MemEnd(0, !0)).await?;
                let m = self.read_msg().await?;
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 300u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["ram_pull: got done "],
                                &match (&m,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Debug::fmt,
                                    )],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                Ok(())
            }
            async fn finish(&mut self) -> Result<(), MigrateError> {
                self.mctx.set_state(MigrationState::Finish).await;
                self.read_ok().await?;
                let _ = self.send_msg(codec::Message::Okay).await;
                Ok(())
            }
            fn end(&mut self) -> Result<(), MigrateError> {
                self.mctx.instance.set_target_state(ReqState::Halt)?;
                if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&self.log(), &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/source.rs",
                                line: 313u32,
                                column: 9u32,
                                function: "",
                                module: "propolis_server::migrate::source",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Info,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["Source Migration Successful"],
                                &match () {
                                    _args => [],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                Ok(())
            }
            async fn read_msg(&mut self) -> Result<codec::Message, MigrateError> {
                self.conn
                    .next()
                    .await
                    .ok_or_else(|| {
                        codec::ProtocolError::Io(io::Error::from(io::ErrorKind::BrokenPipe))
                    })?
                    .map(|msg| match msg {
                        codec::Message::Error(err) => {
                            if ::slog::Level::Error.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&self.log(), &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/migrate/source.rs",
                                                line: 329u32,
                                                column: 21u32,
                                                function: "",
                                                module: "propolis_server::migrate::source",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Error,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["remote error: "],
                                            &match (&err,) {
                                                _args => [::core::fmt::ArgumentV1::new(
                                                    _args.0,
                                                    ::core::fmt::Display::fmt,
                                                )],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                            Err(MigrateError::RemoteError(
                                MigrateRole::Destination,
                                err.to_string(),
                            ))
                        }
                        msg => Ok(msg),
                    })?
            }
            async fn read_ok(&mut self) -> Result<(), MigrateError> {
                match self.read_msg().await? {
                    codec::Message::Okay => Ok(()),
                    msg => {
                        if ::slog::Level::Error.as_usize()
                            <= ::slog::__slog_static_max_level().as_usize()
                        {
                            ::slog::Logger::log(&self.log(), &{
                                #[allow(dead_code)]
                                static RS: ::slog::RecordStatic<'static> = {
                                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                        file: "server/src/lib/migrate/source.rs",
                                        line: 343u32,
                                        column: 17u32,
                                        function: "",
                                        module: "propolis_server::migrate::source",
                                    };
                                    ::slog::RecordStatic {
                                        location: &LOC,
                                        level: ::slog::Level::Error,
                                        tag: "",
                                    }
                                };
                                ::slog::Record::new(
                                    &RS,
                                    &::core::fmt::Arguments::new_v1(
                                        &["expected `Okay` but received: "],
                                        &match (&msg,) {
                                            _args => [::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Debug::fmt,
                                            )],
                                        },
                                    ),
                                    ::slog::BorrowedKV(&()),
                                )
                            })
                        };
                        Err(MigrateError::UnexpectedMessage)
                    }
                }
            }
            async fn read_mem_query(&mut self) -> Result<Range<u64>, MigrateError> {
                match self.read_msg().await? {
                    codec::Message::MemQuery(start, end) => {
                        if start % 4096 != 0 || (end % 4096 != 0 && end != !0) {
                            return Err(MigrateError::Phase);
                        }
                        Ok(start..end)
                    }
                    msg => {
                        if ::slog::Level::Error.as_usize()
                            <= ::slog::__slog_static_max_level().as_usize()
                        {
                            ::slog::Logger::log(&self.log(), &{
                                #[allow(dead_code)]
                                static RS: ::slog::RecordStatic<'static> = {
                                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                        file: "server/src/lib/migrate/source.rs",
                                        line: 358u32,
                                        column: 17u32,
                                        function: "",
                                        module: "propolis_server::migrate::source",
                                    };
                                    ::slog::RecordStatic {
                                        location: &LOC,
                                        level: ::slog::Level::Error,
                                        tag: "",
                                    }
                                };
                                ::slog::Record::new(
                                    &RS,
                                    &::core::fmt::Arguments::new_v1(
                                        &["expected `MemQuery` but received: "],
                                        &match (&msg,) {
                                            _args => [::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Debug::fmt,
                                            )],
                                        },
                                    ),
                                    ::slog::BorrowedKV(&()),
                                )
                            })
                        };
                        Err(MigrateError::UnexpectedMessage)
                    }
                }
            }
            async fn send_msg(&mut self, m: codec::Message) -> Result<(), MigrateError> {
                Ok(self.conn.send(m).await?)
            }
            async fn vmm_ram_bounds(&mut self) -> Result<Range<GuestAddr>, MigrateError> {
                let memctx = self
                    .mctx
                    .async_ctx
                    .dispctx()
                    .await
                    .ok_or(MigrateError::InstanceNotInitialized)?
                    .mctx
                    .memctx();
                memctx
                    .mem_bounds()
                    .ok_or(MigrateError::InvalidInstanceState)
            }
            async fn track_dirty(
                &mut self,
                start_gpa: GuestAddr,
                bits: &mut [u8],
            ) -> Result<(), MigrateError> {
                let handle = self
                    .mctx
                    .async_ctx
                    .dispctx()
                    .await
                    .ok_or(MigrateError::InstanceNotInitialized)?
                    .mctx
                    .hdl();
                handle
                    .track_dirty_pages(start_gpa.0, bits)
                    .map_err(|_| MigrateError::InvalidInstanceState)
            }
            async fn read_guest_mem(
                &mut self,
                addr: GuestAddr,
                buf: &mut [u8],
            ) -> Result<(), MigrateError> {
                let memctx = self
                    .mctx
                    .async_ctx
                    .dispctx()
                    .await
                    .ok_or(MigrateError::InstanceNotInitialized)?
                    .mctx
                    .memctx();
                let len = buf.len();
                memctx.direct_read_into(addr, buf, len);
                Ok(())
            }
        }
    }
    /// Our migration protocol version
    const MIGRATION_PROTOCOL_VERION: usize = 0;
    /// Our migration protocol encoding
    const MIGRATION_PROTOCOL_ENCODING: ProtocolEncoding = ProtocolEncoding::Ron;
    /// The concatenated migration protocol-encoding-version string
    const MIGRATION_PROTOCOL_STR: &'static str = {
        use ::const_format::__cf_osRcTFl4A;
        ({
            #[allow(unused_mut, non_snake_case)]
            const CONCATP_NHPMWYD3NJA: &[__cf_osRcTFl4A::pmr::PArgument] = {
                let fmt = __cf_osRcTFl4A::pmr::FormattingFlags::NEW;
                &[
                    __cf_osRcTFl4A::pmr::PConvWrapper("propolis-migrate-")
                        .to_pargument_display(fmt),
                    __cf_osRcTFl4A::pmr::PConvWrapper(encoding_str(MIGRATION_PROTOCOL_ENCODING))
                        .to_pargument_display(fmt),
                    __cf_osRcTFl4A::pmr::PConvWrapper("/").to_pargument_display(fmt),
                    __cf_osRcTFl4A::pmr::PConvWrapper(MIGRATION_PROTOCOL_VERION)
                        .to_pargument_display(fmt),
                ]
            };
            {
                const ARR_LEN: usize =
                    ::const_format::pmr::PArgument::calc_len(CONCATP_NHPMWYD3NJA);
                const CONCAT_ARR: &::const_format::pmr::LenAndArray<[u8; ARR_LEN]> = {
                    use ::const_format::{__write_pvariant, pmr::PVariant};
                    let mut out = ::const_format::pmr::LenAndArray {
                        len: 0,
                        array: [0u8; ARR_LEN],
                    };
                    let input = CONCATP_NHPMWYD3NJA;
                    {
                        let ::const_format::pmr::Range {
                            start: mut outer_i,
                            end,
                        } = 0..input.len();
                        while outer_i < end {
                            {
                                let current = &input[outer_i];
                                match current.elem {
                                    PVariant::Str(s) => {
                                        let str = s.as_bytes();
                                        let is_display = current.fmt.is_display();
                                        let mut i = 0;
                                        if is_display {
                                            while i < str.len() {
                                                out.array[out.len] = str[i];
                                                out.len += 1;
                                                i += 1;
                                            }
                                        } else {
                                            out.array[out.len] = b'"';
                                            out.len += 1;
                                            while i < str.len() {
                                                use ::const_format::pmr::{
                                                    hex_as_ascii, ForEscaping, FOR_ESCAPING,
                                                };
                                                let c = str[i];
                                                let mut written_c = c;
                                                if c < 128 {
                                                    let shifted = 1 << c;
                                                    if (FOR_ESCAPING.is_escaped & shifted) != 0 {
                                                        out.array[out.len] = b'\\';
                                                        out.len += 1;
                                                        if (FOR_ESCAPING.is_backslash_escaped
                                                            & shifted)
                                                            == 0
                                                        {
                                                            out.array[out.len] = b'x';
                                                            out.array[out.len + 1] =
                                                                hex_as_ascii(c >> 4);
                                                            out.len += 2;
                                                            written_c = hex_as_ascii(c & 0b1111);
                                                        } else {
                                                            written_c =
                                                                ForEscaping::get_backslash_escape(
                                                                    c,
                                                                );
                                                        };
                                                    }
                                                }
                                                out.array[out.len] = written_c;
                                                out.len += 1;
                                                i += 1;
                                            }
                                            out.array[out.len] = b'"';
                                            out.len += 1;
                                        }
                                    }
                                    PVariant::Int(int) => {
                                        let wrapper = ::const_format::pmr::PWrapper(int);
                                        let debug_display;
                                        let bin;
                                        let hex;
                                        let sa : & :: const_format :: pmr :: StartAndArray < [_] > = match current . fmt { :: const_format :: pmr :: Formatting :: Display => { debug_display = wrapper . to_start_array_display () ; & debug_display } :: const_format :: pmr :: Formatting :: Debug => match current . fmt_flags . num_fmt () { :: const_format :: pmr :: NumberFormatting :: Decimal => { debug_display = wrapper . to_start_array_debug () ; & debug_display } :: const_format :: pmr :: NumberFormatting :: Binary => { bin = wrapper . to_start_array_binary (current . fmt_flags) ; & bin } :: const_format :: pmr :: NumberFormatting :: Hexadecimal => { hex = wrapper . to_start_array_hexadecimal (current . fmt_flags) ; & hex } } , } ;
                                        let mut start = sa.start;
                                        while start < sa.array.len() {
                                            out.array[out.len] = sa.array[start];
                                            out.len += 1;
                                            start += 1;
                                        }
                                    }
                                    PVariant::Char(c) => {
                                        let encoded = c.encoded();
                                        let len = c.len();
                                        let mut start = 0;
                                        while start < len {
                                            out.array[out.len] = encoded[start];
                                            out.len += 1;
                                            start += 1;
                                        }
                                    }
                                }
                            }
                            outer_i += 1;
                        }
                    }
                    &{ out }
                };
                #[allow(clippy::transmute_ptr_to_ptr)]
                const CONCAT_STR: &str = unsafe {
                    let slice = ::const_format::pmr::transmute::<
                        &[u8; ARR_LEN],
                        &[u8; CONCAT_ARR.len],
                    >(&CONCAT_ARR.array);
                    {
                        let bytes: &'static [::const_format::pmr::u8] = slice;
                        let string: &'static ::const_format::pmr::str = {
                            ::const_format::__hidden_utils::PtrToRef {
                                ptr: bytes as *const [::const_format::pmr::u8] as *const str,
                            }
                            .reff
                        };
                        string
                    }
                };
                CONCAT_STR
            }
        })
    };
    /// Supported encoding formats
    enum ProtocolEncoding {
        Ron,
    }
    /// Small helper function to stringify ProtocolEncoding
    const fn encoding_str(e: ProtocolEncoding) -> &'static str {
        match e {
            ProtocolEncoding::Ron => "ron",
        }
    }
    /// Context created as part of a migration.
    pub struct MigrateContext {
        /// The external ID used to identify a migration across both the source and destination.
        migration_id: Uuid,
        /// The current state of the migration process on this Instance.
        state: RwLock<MigrationState>,
        /// A handle to the underlying propolis [`Instance`].
        instance: Arc<Instance>,
        /// Async descriptor context for the migrate task to access machine state in async context.
        async_ctx: AsyncCtx,
        /// Logger for migration created from initial migration request.
        log: slog::Logger,
    }
    impl MigrateContext {
        fn new(migration_id: Uuid, instance: Arc<Instance>, log: slog::Logger) -> MigrateContext {
            MigrateContext {
                migration_id,
                state: RwLock::new(MigrationState::Sync),
                async_ctx: instance.disp.async_ctx(),
                instance,
                log,
            }
        }
        async fn get_state(&self) -> MigrationState {
            let state = self.state.read().await;
            *state
        }
        async fn set_state(&self, new: MigrationState) {
            let mut state = self.state.write().await;
            *state = new;
        }
    }
    pub struct MigrateTask {
        #[allow(dead_code)]
        task: JoinHandle<()>,
        context: Arc<MigrateContext>,
    }
    /// Errors which may occur during the course of a migration
    pub enum MigrateError {
        /// An error as a result of some HTTP operation (i.e. trying to establish
        /// the websocket connection between the source and destination)
        #[error("HTTP error: {0}")]
        Http(String),
        /// Failed to initiate the migration protocol
        #[error("couldn't establish migration connection to source instance")]
        Initiate,
        /// The source and destination instances are not compatible
        #[error("the source ({0}) and destination ({1}) instances are incompatible for migration")]
        Incompatible(String, String),
        /// Incomplete WebSocket upgrade request
        #[error("expected connection upgrade")]
        UpgradeExpected,
        /// Attempted to migrate an uninitialized instance
        #[error("instance is not initialized")]
        InstanceNotInitialized,
        /// The given UUID does not match the existing instance/migration UUID
        #[error("unexpected Uuid")]
        UuidMismatch,
        /// A different migration already in progress
        #[error("a migration from the current instance is already in progress")]
        MigrationAlreadyInProgress,
        /// Migration state was requested with no migration in process
        #[error("no migration is currently in progress")]
        NoMigrationInProgress,
        /// Encountered an error as part of encoding/decoding migration messages
        #[error("codec error: {0}")]
        Codec(String),
        /// The instance is in an invalid state for the current operation
        #[error("encountered invalid instance state")]
        InvalidInstanceState,
        /// Received a message out of order
        #[error("received unexpected migration message")]
        UnexpectedMessage,
        /// Failed to pause the source instance's devices or tasks
        #[error("failed to pause source instance")]
        SourcePause,
        /// Phase error
        #[error("received out-of-phase message")]
        Phase,
        /// Failed to export/import device state for migration
        #[error("failed to migrate device state: {0}")]
        DeviceState(#[from] MigrateStateError),
        /// The destination instance doesn't recognize the received device
        #[error("received device state for unknown device ({0})")]
        UnknownDevice(String),
        /// The other end of the migration ran into an error
        #[error("{0} migration instance encountered error: {1}")]
        RemoteError(MigrateRole, String),
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for MigrateError {
        #[inline]
        fn clone(&self) -> MigrateError {
            match (&*self,) {
                (&MigrateError::Http(ref __self_0),) => {
                    MigrateError::Http(::core::clone::Clone::clone(&(*__self_0)))
                }
                (&MigrateError::Initiate,) => MigrateError::Initiate,
                (&MigrateError::Incompatible(ref __self_0, ref __self_1),) => {
                    MigrateError::Incompatible(
                        ::core::clone::Clone::clone(&(*__self_0)),
                        ::core::clone::Clone::clone(&(*__self_1)),
                    )
                }
                (&MigrateError::UpgradeExpected,) => MigrateError::UpgradeExpected,
                (&MigrateError::InstanceNotInitialized,) => MigrateError::InstanceNotInitialized,
                (&MigrateError::UuidMismatch,) => MigrateError::UuidMismatch,
                (&MigrateError::MigrationAlreadyInProgress,) => {
                    MigrateError::MigrationAlreadyInProgress
                }
                (&MigrateError::NoMigrationInProgress,) => MigrateError::NoMigrationInProgress,
                (&MigrateError::Codec(ref __self_0),) => {
                    MigrateError::Codec(::core::clone::Clone::clone(&(*__self_0)))
                }
                (&MigrateError::InvalidInstanceState,) => MigrateError::InvalidInstanceState,
                (&MigrateError::UnexpectedMessage,) => MigrateError::UnexpectedMessage,
                (&MigrateError::SourcePause,) => MigrateError::SourcePause,
                (&MigrateError::Phase,) => MigrateError::Phase,
                (&MigrateError::DeviceState(ref __self_0),) => {
                    MigrateError::DeviceState(::core::clone::Clone::clone(&(*__self_0)))
                }
                (&MigrateError::UnknownDevice(ref __self_0),) => {
                    MigrateError::UnknownDevice(::core::clone::Clone::clone(&(*__self_0)))
                }
                (&MigrateError::RemoteError(ref __self_0, ref __self_1),) => {
                    MigrateError::RemoteError(
                        ::core::clone::Clone::clone(&(*__self_0)),
                        ::core::clone::Clone::clone(&(*__self_1)),
                    )
                }
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for MigrateError {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match (&*self,) {
                (&MigrateError::Http(ref __self_0),) => {
                    let debug_trait_builder = &mut ::core::fmt::Formatter::debug_tuple(f, "Http");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&MigrateError::Initiate,) => ::core::fmt::Formatter::write_str(f, "Initiate"),
                (&MigrateError::Incompatible(ref __self_0, ref __self_1),) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "Incompatible");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&MigrateError::UpgradeExpected,) => {
                    ::core::fmt::Formatter::write_str(f, "UpgradeExpected")
                }
                (&MigrateError::InstanceNotInitialized,) => {
                    ::core::fmt::Formatter::write_str(f, "InstanceNotInitialized")
                }
                (&MigrateError::UuidMismatch,) => {
                    ::core::fmt::Formatter::write_str(f, "UuidMismatch")
                }
                (&MigrateError::MigrationAlreadyInProgress,) => {
                    ::core::fmt::Formatter::write_str(f, "MigrationAlreadyInProgress")
                }
                (&MigrateError::NoMigrationInProgress,) => {
                    ::core::fmt::Formatter::write_str(f, "NoMigrationInProgress")
                }
                (&MigrateError::Codec(ref __self_0),) => {
                    let debug_trait_builder = &mut ::core::fmt::Formatter::debug_tuple(f, "Codec");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&MigrateError::InvalidInstanceState,) => {
                    ::core::fmt::Formatter::write_str(f, "InvalidInstanceState")
                }
                (&MigrateError::UnexpectedMessage,) => {
                    ::core::fmt::Formatter::write_str(f, "UnexpectedMessage")
                }
                (&MigrateError::SourcePause,) => {
                    ::core::fmt::Formatter::write_str(f, "SourcePause")
                }
                (&MigrateError::Phase,) => ::core::fmt::Formatter::write_str(f, "Phase"),
                (&MigrateError::DeviceState(ref __self_0),) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "DeviceState");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&MigrateError::UnknownDevice(ref __self_0),) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "UnknownDevice");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&MigrateError::RemoteError(ref __self_0, ref __self_1),) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "RemoteError");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_1));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
            }
        }
    }
    #[allow(unused_qualifications)]
    impl std::error::Error for MigrateError {
        fn source(&self) -> std::option::Option<&(dyn std::error::Error + 'static)> {
            use thiserror::private::AsDynError;
            #[allow(deprecated)]
            match self {
                MigrateError::Http { .. } => std::option::Option::None,
                MigrateError::Initiate { .. } => std::option::Option::None,
                MigrateError::Incompatible { .. } => std::option::Option::None,
                MigrateError::UpgradeExpected { .. } => std::option::Option::None,
                MigrateError::InstanceNotInitialized { .. } => std::option::Option::None,
                MigrateError::UuidMismatch { .. } => std::option::Option::None,
                MigrateError::MigrationAlreadyInProgress { .. } => std::option::Option::None,
                MigrateError::NoMigrationInProgress { .. } => std::option::Option::None,
                MigrateError::Codec { .. } => std::option::Option::None,
                MigrateError::InvalidInstanceState { .. } => std::option::Option::None,
                MigrateError::UnexpectedMessage { .. } => std::option::Option::None,
                MigrateError::SourcePause { .. } => std::option::Option::None,
                MigrateError::Phase { .. } => std::option::Option::None,
                MigrateError::DeviceState { 0: source, .. } => {
                    std::option::Option::Some(source.as_dyn_error())
                }
                MigrateError::UnknownDevice { .. } => std::option::Option::None,
                MigrateError::RemoteError { .. } => std::option::Option::None,
            }
        }
    }
    #[allow(unused_qualifications)]
    impl std::fmt::Display for MigrateError {
        fn fmt(&self, __formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            #[allow(unused_imports)]
            use thiserror::private::{DisplayAsDisplay, PathAsDisplay};
            #[allow(unused_variables, deprecated, clippy::used_underscore_binding)]
            match self {
                MigrateError::Http(_0) => __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                    &["HTTP error: "],
                    &match (&_0.as_display(),) {
                        _args => [::core::fmt::ArgumentV1::new(
                            _args.0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                )),
                MigrateError::Initiate {} => __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                    &["couldn\'t establish migration connection to source instance"],
                    &match () {
                        _args => [],
                    },
                )),
                MigrateError::Incompatible(_0, _1) => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &[
                            "the source (",
                            ") and destination (",
                            ") instances are incompatible for migration",
                        ],
                        &match (&_0.as_display(), &_1.as_display()) {
                            _args => [
                                ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                                ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                            ],
                        },
                    ))
                }
                MigrateError::UpgradeExpected {} => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["expected connection upgrade"],
                        &match () {
                            _args => [],
                        },
                    ))
                }
                MigrateError::InstanceNotInitialized {} => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["instance is not initialized"],
                        &match () {
                            _args => [],
                        },
                    ))
                }
                MigrateError::UuidMismatch {} => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["unexpected Uuid"],
                        &match () {
                            _args => [],
                        },
                    ))
                }
                MigrateError::MigrationAlreadyInProgress {} => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["a migration from the current instance is already in progress"],
                        &match () {
                            _args => [],
                        },
                    ))
                }
                MigrateError::NoMigrationInProgress {} => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["no migration is currently in progress"],
                        &match () {
                            _args => [],
                        },
                    ))
                }
                MigrateError::Codec(_0) => __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                    &["codec error: "],
                    &match (&_0.as_display(),) {
                        _args => [::core::fmt::ArgumentV1::new(
                            _args.0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                )),
                MigrateError::InvalidInstanceState {} => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["encountered invalid instance state"],
                        &match () {
                            _args => [],
                        },
                    ))
                }
                MigrateError::UnexpectedMessage {} => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["received unexpected migration message"],
                        &match () {
                            _args => [],
                        },
                    ))
                }
                MigrateError::SourcePause {} => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["failed to pause source instance"],
                        &match () {
                            _args => [],
                        },
                    ))
                }
                MigrateError::Phase {} => __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                    &["received out-of-phase message"],
                    &match () {
                        _args => [],
                    },
                )),
                MigrateError::DeviceState(_0) => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["failed to migrate device state: "],
                        &match (&_0.as_display(),) {
                            _args => [::core::fmt::ArgumentV1::new(
                                _args.0,
                                ::core::fmt::Display::fmt,
                            )],
                        },
                    ))
                }
                MigrateError::UnknownDevice(_0) => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["received device state for unknown device (", ")"],
                        &match (&_0.as_display(),) {
                            _args => [::core::fmt::ArgumentV1::new(
                                _args.0,
                                ::core::fmt::Display::fmt,
                            )],
                        },
                    ))
                }
                MigrateError::RemoteError(_0, _1) => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["", " migration instance encountered error: "],
                        &match (&_0.as_display(), &_1.as_display()) {
                            _args => [
                                ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                                ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                            ],
                        },
                    ))
                }
            }
        }
    }
    #[allow(unused_qualifications)]
    impl std::convert::From<MigrateStateError> for MigrateError {
        #[allow(deprecated)]
        fn from(source: MigrateStateError) -> Self {
            MigrateError::DeviceState { 0: source }
        }
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for MigrateError {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field {
                    __field0,
                    __field1,
                    __field2,
                    __field3,
                    __field4,
                    __field5,
                    __field6,
                    __field7,
                    __field8,
                    __field9,
                    __field10,
                    __field11,
                    __field12,
                    __field13,
                    __field14,
                    __field15,
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "variant identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            2u64 => _serde::__private::Ok(__Field::__field2),
                            3u64 => _serde::__private::Ok(__Field::__field3),
                            4u64 => _serde::__private::Ok(__Field::__field4),
                            5u64 => _serde::__private::Ok(__Field::__field5),
                            6u64 => _serde::__private::Ok(__Field::__field6),
                            7u64 => _serde::__private::Ok(__Field::__field7),
                            8u64 => _serde::__private::Ok(__Field::__field8),
                            9u64 => _serde::__private::Ok(__Field::__field9),
                            10u64 => _serde::__private::Ok(__Field::__field10),
                            11u64 => _serde::__private::Ok(__Field::__field11),
                            12u64 => _serde::__private::Ok(__Field::__field12),
                            13u64 => _serde::__private::Ok(__Field::__field13),
                            14u64 => _serde::__private::Ok(__Field::__field14),
                            15u64 => _serde::__private::Ok(__Field::__field15),
                            _ => _serde::__private::Err(_serde::de::Error::invalid_value(
                                _serde::de::Unexpected::Unsigned(__value),
                                &"variant index 0 <= i < 16",
                            )),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "Http" => _serde::__private::Ok(__Field::__field0),
                            "Initiate" => _serde::__private::Ok(__Field::__field1),
                            "Incompatible" => _serde::__private::Ok(__Field::__field2),
                            "UpgradeExpected" => _serde::__private::Ok(__Field::__field3),
                            "InstanceNotInitialized" => _serde::__private::Ok(__Field::__field4),
                            "UuidMismatch" => _serde::__private::Ok(__Field::__field5),
                            "MigrationAlreadyInProgress" => {
                                _serde::__private::Ok(__Field::__field6)
                            }
                            "NoMigrationInProgress" => _serde::__private::Ok(__Field::__field7),
                            "Codec" => _serde::__private::Ok(__Field::__field8),
                            "InvalidInstanceState" => _serde::__private::Ok(__Field::__field9),
                            "UnexpectedMessage" => _serde::__private::Ok(__Field::__field10),
                            "SourcePause" => _serde::__private::Ok(__Field::__field11),
                            "Phase" => _serde::__private::Ok(__Field::__field12),
                            "DeviceState" => _serde::__private::Ok(__Field::__field13),
                            "UnknownDevice" => _serde::__private::Ok(__Field::__field14),
                            "RemoteError" => _serde::__private::Ok(__Field::__field15),
                            _ => _serde::__private::Err(_serde::de::Error::unknown_variant(
                                __value, VARIANTS,
                            )),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"Http" => _serde::__private::Ok(__Field::__field0),
                            b"Initiate" => _serde::__private::Ok(__Field::__field1),
                            b"Incompatible" => _serde::__private::Ok(__Field::__field2),
                            b"UpgradeExpected" => _serde::__private::Ok(__Field::__field3),
                            b"InstanceNotInitialized" => _serde::__private::Ok(__Field::__field4),
                            b"UuidMismatch" => _serde::__private::Ok(__Field::__field5),
                            b"MigrationAlreadyInProgress" => {
                                _serde::__private::Ok(__Field::__field6)
                            }
                            b"NoMigrationInProgress" => _serde::__private::Ok(__Field::__field7),
                            b"Codec" => _serde::__private::Ok(__Field::__field8),
                            b"InvalidInstanceState" => _serde::__private::Ok(__Field::__field9),
                            b"UnexpectedMessage" => _serde::__private::Ok(__Field::__field10),
                            b"SourcePause" => _serde::__private::Ok(__Field::__field11),
                            b"Phase" => _serde::__private::Ok(__Field::__field12),
                            b"DeviceState" => _serde::__private::Ok(__Field::__field13),
                            b"UnknownDevice" => _serde::__private::Ok(__Field::__field14),
                            b"RemoteError" => _serde::__private::Ok(__Field::__field15),
                            _ => {
                                let __value = &_serde::__private::from_utf8_lossy(__value);
                                _serde::__private::Err(_serde::de::Error::unknown_variant(
                                    __value, VARIANTS,
                                ))
                            }
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<MigrateError>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = MigrateError;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "enum MigrateError")
                    }
                    fn visit_enum<__A>(
                        self,
                        __data: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::EnumAccess<'de>,
                    {
                        match match _serde::de::EnumAccess::variant(__data) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            (__Field::__field0, __variant) => _serde::__private::Result::map(
                                _serde::de::VariantAccess::newtype_variant::<String>(__variant),
                                MigrateError::Http,
                            ),
                            (__Field::__field1, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::Initiate)
                            }
                            (__Field::__field2, __variant) => {
                                struct __Visitor<'de> {
                                    marker: _serde::__private::PhantomData<MigrateError>,
                                    lifetime: _serde::__private::PhantomData<&'de ()>,
                                }
                                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                                    type Value = MigrateError;
                                    fn expecting(
                                        &self,
                                        __formatter: &mut _serde::__private::Formatter,
                                    ) -> _serde::__private::fmt::Result
                                    {
                                        _serde::__private::Formatter::write_str(
                                            __formatter,
                                            "tuple variant MigrateError::Incompatible",
                                        )
                                    }
                                    #[inline]
                                    fn visit_seq<__A>(
                                        self,
                                        mut __seq: __A,
                                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                                    where
                                        __A: _serde::de::SeqAccess<'de>,
                                    {
                                        let __field0 =
                                            match match _serde::de::SeqAccess::next_element::<String>(
                                                &mut __seq,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            } {
                                                _serde::__private::Some(__value) => __value,
                                                _serde::__private::None => {
                                                    return _serde :: __private :: Err (_serde :: de :: Error :: invalid_length (0usize , & "tuple variant MigrateError::Incompatible with 2 elements")) ;
                                                }
                                            };
                                        let __field1 =
                                            match match _serde::de::SeqAccess::next_element::<String>(
                                                &mut __seq,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            } {
                                                _serde::__private::Some(__value) => __value,
                                                _serde::__private::None => {
                                                    return _serde :: __private :: Err (_serde :: de :: Error :: invalid_length (1usize , & "tuple variant MigrateError::Incompatible with 2 elements")) ;
                                                }
                                            };
                                        _serde::__private::Ok(MigrateError::Incompatible(
                                            __field0, __field1,
                                        ))
                                    }
                                }
                                _serde::de::VariantAccess::tuple_variant(
                                    __variant,
                                    2usize,
                                    __Visitor {
                                        marker: _serde::__private::PhantomData::<MigrateError>,
                                        lifetime: _serde::__private::PhantomData,
                                    },
                                )
                            }
                            (__Field::__field3, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::UpgradeExpected)
                            }
                            (__Field::__field4, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::InstanceNotInitialized)
                            }
                            (__Field::__field5, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::UuidMismatch)
                            }
                            (__Field::__field6, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::MigrationAlreadyInProgress)
                            }
                            (__Field::__field7, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::NoMigrationInProgress)
                            }
                            (__Field::__field8, __variant) => _serde::__private::Result::map(
                                _serde::de::VariantAccess::newtype_variant::<String>(__variant),
                                MigrateError::Codec,
                            ),
                            (__Field::__field9, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::InvalidInstanceState)
                            }
                            (__Field::__field10, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::UnexpectedMessage)
                            }
                            (__Field::__field11, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::SourcePause)
                            }
                            (__Field::__field12, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(MigrateError::Phase)
                            }
                            (__Field::__field13, __variant) => _serde::__private::Result::map(
                                _serde::de::VariantAccess::newtype_variant::<MigrateStateError>(
                                    __variant,
                                ),
                                MigrateError::DeviceState,
                            ),
                            (__Field::__field14, __variant) => _serde::__private::Result::map(
                                _serde::de::VariantAccess::newtype_variant::<String>(__variant),
                                MigrateError::UnknownDevice,
                            ),
                            (__Field::__field15, __variant) => {
                                struct __Visitor<'de> {
                                    marker: _serde::__private::PhantomData<MigrateError>,
                                    lifetime: _serde::__private::PhantomData<&'de ()>,
                                }
                                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                                    type Value = MigrateError;
                                    fn expecting(
                                        &self,
                                        __formatter: &mut _serde::__private::Formatter,
                                    ) -> _serde::__private::fmt::Result
                                    {
                                        _serde::__private::Formatter::write_str(
                                            __formatter,
                                            "tuple variant MigrateError::RemoteError",
                                        )
                                    }
                                    #[inline]
                                    fn visit_seq<__A>(
                                        self,
                                        mut __seq: __A,
                                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                                    where
                                        __A: _serde::de::SeqAccess<'de>,
                                    {
                                        let __field0 =
                                            match match _serde::de::SeqAccess::next_element::<
                                                MigrateRole,
                                            >(
                                                &mut __seq
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            } {
                                                _serde::__private::Some(__value) => __value,
                                                _serde::__private::None => {
                                                    return _serde :: __private :: Err (_serde :: de :: Error :: invalid_length (0usize , & "tuple variant MigrateError::RemoteError with 2 elements")) ;
                                                }
                                            };
                                        let __field1 =
                                            match match _serde::de::SeqAccess::next_element::<String>(
                                                &mut __seq,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            } {
                                                _serde::__private::Some(__value) => __value,
                                                _serde::__private::None => {
                                                    return _serde :: __private :: Err (_serde :: de :: Error :: invalid_length (1usize , & "tuple variant MigrateError::RemoteError with 2 elements")) ;
                                                }
                                            };
                                        _serde::__private::Ok(MigrateError::RemoteError(
                                            __field0, __field1,
                                        ))
                                    }
                                }
                                _serde::de::VariantAccess::tuple_variant(
                                    __variant,
                                    2usize,
                                    __Visitor {
                                        marker: _serde::__private::PhantomData::<MigrateError>,
                                        lifetime: _serde::__private::PhantomData,
                                    },
                                )
                            }
                        }
                    }
                }
                const VARIANTS: &'static [&'static str] = &[
                    "Http",
                    "Initiate",
                    "Incompatible",
                    "UpgradeExpected",
                    "InstanceNotInitialized",
                    "UuidMismatch",
                    "MigrationAlreadyInProgress",
                    "NoMigrationInProgress",
                    "Codec",
                    "InvalidInstanceState",
                    "UnexpectedMessage",
                    "SourcePause",
                    "Phase",
                    "DeviceState",
                    "UnknownDevice",
                    "RemoteError",
                ];
                _serde::Deserializer::deserialize_enum(
                    __deserializer,
                    "MigrateError",
                    VARIANTS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<MigrateError>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    impl ::core::marker::StructuralPartialEq for MigrateError {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for MigrateError {
        #[inline]
        fn eq(&self, other: &MigrateError) -> bool {
            {
                let __self_vi = ::core::intrinsics::discriminant_value(&*self);
                let __arg_1_vi = ::core::intrinsics::discriminant_value(&*other);
                if true && __self_vi == __arg_1_vi {
                    match (&*self, &*other) {
                        (&MigrateError::Http(ref __self_0), &MigrateError::Http(ref __arg_1_0)) => {
                            (*__self_0) == (*__arg_1_0)
                        }
                        (
                            &MigrateError::Incompatible(ref __self_0, ref __self_1),
                            &MigrateError::Incompatible(ref __arg_1_0, ref __arg_1_1),
                        ) => (*__self_0) == (*__arg_1_0) && (*__self_1) == (*__arg_1_1),
                        (
                            &MigrateError::Codec(ref __self_0),
                            &MigrateError::Codec(ref __arg_1_0),
                        ) => (*__self_0) == (*__arg_1_0),
                        (
                            &MigrateError::DeviceState(ref __self_0),
                            &MigrateError::DeviceState(ref __arg_1_0),
                        ) => (*__self_0) == (*__arg_1_0),
                        (
                            &MigrateError::UnknownDevice(ref __self_0),
                            &MigrateError::UnknownDevice(ref __arg_1_0),
                        ) => (*__self_0) == (*__arg_1_0),
                        (
                            &MigrateError::RemoteError(ref __self_0, ref __self_1),
                            &MigrateError::RemoteError(ref __arg_1_0, ref __arg_1_1),
                        ) => (*__self_0) == (*__arg_1_0) && (*__self_1) == (*__arg_1_1),
                        _ => true,
                    }
                } else {
                    false
                }
            }
        }
        #[inline]
        fn ne(&self, other: &MigrateError) -> bool {
            {
                let __self_vi = ::core::intrinsics::discriminant_value(&*self);
                let __arg_1_vi = ::core::intrinsics::discriminant_value(&*other);
                if true && __self_vi == __arg_1_vi {
                    match (&*self, &*other) {
                        (&MigrateError::Http(ref __self_0), &MigrateError::Http(ref __arg_1_0)) => {
                            (*__self_0) != (*__arg_1_0)
                        }
                        (
                            &MigrateError::Incompatible(ref __self_0, ref __self_1),
                            &MigrateError::Incompatible(ref __arg_1_0, ref __arg_1_1),
                        ) => (*__self_0) != (*__arg_1_0) || (*__self_1) != (*__arg_1_1),
                        (
                            &MigrateError::Codec(ref __self_0),
                            &MigrateError::Codec(ref __arg_1_0),
                        ) => (*__self_0) != (*__arg_1_0),
                        (
                            &MigrateError::DeviceState(ref __self_0),
                            &MigrateError::DeviceState(ref __arg_1_0),
                        ) => (*__self_0) != (*__arg_1_0),
                        (
                            &MigrateError::UnknownDevice(ref __self_0),
                            &MigrateError::UnknownDevice(ref __arg_1_0),
                        ) => (*__self_0) != (*__arg_1_0),
                        (
                            &MigrateError::RemoteError(ref __self_0, ref __self_1),
                            &MigrateError::RemoteError(ref __arg_1_0, ref __arg_1_1),
                        ) => (*__self_0) != (*__arg_1_0) || (*__self_1) != (*__arg_1_1),
                        _ => false,
                    }
                } else {
                    true
                }
            }
        }
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for MigrateError {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                match *self {
                    MigrateError::Http(ref __field0) => {
                        _serde::Serializer::serialize_newtype_variant(
                            __serializer,
                            "MigrateError",
                            0u32,
                            "Http",
                            __field0,
                        )
                    }
                    MigrateError::Initiate => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "MigrateError",
                        1u32,
                        "Initiate",
                    ),
                    MigrateError::Incompatible(ref __field0, ref __field1) => {
                        let mut __serde_state = match _serde::Serializer::serialize_tuple_variant(
                            __serializer,
                            "MigrateError",
                            2u32,
                            "Incompatible",
                            0 + 1 + 1,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        match _serde::ser::SerializeTupleVariant::serialize_field(
                            &mut __serde_state,
                            __field0,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        match _serde::ser::SerializeTupleVariant::serialize_field(
                            &mut __serde_state,
                            __field1,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        _serde::ser::SerializeTupleVariant::end(__serde_state)
                    }
                    MigrateError::UpgradeExpected => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "MigrateError",
                        3u32,
                        "UpgradeExpected",
                    ),
                    MigrateError::InstanceNotInitialized => {
                        _serde::Serializer::serialize_unit_variant(
                            __serializer,
                            "MigrateError",
                            4u32,
                            "InstanceNotInitialized",
                        )
                    }
                    MigrateError::UuidMismatch => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "MigrateError",
                        5u32,
                        "UuidMismatch",
                    ),
                    MigrateError::MigrationAlreadyInProgress => {
                        _serde::Serializer::serialize_unit_variant(
                            __serializer,
                            "MigrateError",
                            6u32,
                            "MigrationAlreadyInProgress",
                        )
                    }
                    MigrateError::NoMigrationInProgress => {
                        _serde::Serializer::serialize_unit_variant(
                            __serializer,
                            "MigrateError",
                            7u32,
                            "NoMigrationInProgress",
                        )
                    }
                    MigrateError::Codec(ref __field0) => {
                        _serde::Serializer::serialize_newtype_variant(
                            __serializer,
                            "MigrateError",
                            8u32,
                            "Codec",
                            __field0,
                        )
                    }
                    MigrateError::InvalidInstanceState => {
                        _serde::Serializer::serialize_unit_variant(
                            __serializer,
                            "MigrateError",
                            9u32,
                            "InvalidInstanceState",
                        )
                    }
                    MigrateError::UnexpectedMessage => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "MigrateError",
                        10u32,
                        "UnexpectedMessage",
                    ),
                    MigrateError::SourcePause => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "MigrateError",
                        11u32,
                        "SourcePause",
                    ),
                    MigrateError::Phase => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "MigrateError",
                        12u32,
                        "Phase",
                    ),
                    MigrateError::DeviceState(ref __field0) => {
                        _serde::Serializer::serialize_newtype_variant(
                            __serializer,
                            "MigrateError",
                            13u32,
                            "DeviceState",
                            __field0,
                        )
                    }
                    MigrateError::UnknownDevice(ref __field0) => {
                        _serde::Serializer::serialize_newtype_variant(
                            __serializer,
                            "MigrateError",
                            14u32,
                            "UnknownDevice",
                            __field0,
                        )
                    }
                    MigrateError::RemoteError(ref __field0, ref __field1) => {
                        let mut __serde_state = match _serde::Serializer::serialize_tuple_variant(
                            __serializer,
                            "MigrateError",
                            15u32,
                            "RemoteError",
                            0 + 1 + 1,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        match _serde::ser::SerializeTupleVariant::serialize_field(
                            &mut __serde_state,
                            __field0,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        match _serde::ser::SerializeTupleVariant::serialize_field(
                            &mut __serde_state,
                            __field1,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        _serde::ser::SerializeTupleVariant::end(__serde_state)
                    }
                }
            }
        }
    };
    impl MigrateError {
        fn incompatible(src: &str, dst: &str) -> MigrateError {
            MigrateError::Incompatible(src.to_string(), dst.to_string())
        }
    }
    impl From<hyper::Error> for MigrateError {
        fn from(err: hyper::Error) -> MigrateError {
            MigrateError::Http(err.to_string())
        }
    }
    impl From<TransitionError> for MigrateError {
        fn from(err: TransitionError) -> Self {
            match err {
                TransitionError::ResetWhileHalted
                | TransitionError::InvalidTarget { .. }
                | TransitionError::Terminal => MigrateError::InvalidInstanceState,
                TransitionError::MigrationAlreadyInProgress => {
                    MigrateError::MigrationAlreadyInProgress
                }
            }
        }
    }
    impl From<codec::ProtocolError> for MigrateError {
        fn from(err: codec::ProtocolError) -> Self {
            MigrateError::Codec(err.to_string())
        }
    }
    impl Into<HttpError> for MigrateError {
        fn into(self) -> HttpError {
            let msg = {
                let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                    &["migration failed: "],
                    &match (&self,) {
                        _args => [::core::fmt::ArgumentV1::new(
                            _args.0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                ));
                res
            };
            match &self {
                MigrateError::Http(_)
                | MigrateError::Initiate
                | MigrateError::Incompatible(_, _)
                | MigrateError::InstanceNotInitialized
                | MigrateError::InvalidInstanceState
                | MigrateError::Codec(_)
                | MigrateError::UnexpectedMessage
                | MigrateError::SourcePause
                | MigrateError::Phase
                | MigrateError::DeviceState(_)
                | MigrateError::RemoteError(_, _) => HttpError::for_internal_error(msg),
                MigrateError::MigrationAlreadyInProgress
                | MigrateError::NoMigrationInProgress
                | MigrateError::UuidMismatch
                | MigrateError::UpgradeExpected
                | MigrateError::UnknownDevice(_) => HttpError::for_bad_request(None, msg),
            }
        }
    }
    /// Serialized device state sent during migration.
    struct Device {
        /// The unique name identifying the device in the instance inventory.
        instance_name: String,
        /// The (Ron) serialized device state.
        /// See `Migrate::export`.
        payload: String,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for Device {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                Device {
                    instance_name: ref __self_0_0,
                    payload: ref __self_0_1,
                } => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_struct(f, "Device");
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "instance_name",
                        &&(*__self_0_0),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "payload",
                        &&(*__self_0_1),
                    );
                    ::core::fmt::DebugStruct::finish(debug_trait_builder)
                }
            }
        }
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for Device {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field {
                    __field0,
                    __field1,
                    __ignore,
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "field identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "instance_name" => _serde::__private::Ok(__Field::__field0),
                            "payload" => _serde::__private::Ok(__Field::__field1),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"instance_name" => _serde::__private::Ok(__Field::__field0),
                            b"payload" => _serde::__private::Ok(__Field::__field1),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<Device>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = Device;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "struct Device")
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 =
                            match match _serde::de::SeqAccess::next_element::<String>(&mut __seq) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct Device with 2 elements",
                                        ),
                                    );
                                }
                            };
                        let __field1 =
                            match match _serde::de::SeqAccess::next_element::<String>(&mut __seq) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            1usize,
                                            &"struct Device with 2 elements",
                                        ),
                                    );
                                }
                            };
                        _serde::__private::Ok(Device {
                            instance_name: __field0,
                            payload: __field1,
                        })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<String> =
                            _serde::__private::None;
                        let mut __field1: _serde::__private::Option<String> =
                            _serde::__private::None;
                        while let _serde::__private::Some(__key) =
                            match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            }
                        {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "instance_name",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<String>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field1 => {
                                    if _serde::__private::Option::is_some(&__field1) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "payload",
                                            ),
                                        );
                                    }
                                    __field1 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<String>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                _ => {
                                    let _ = match _serde::de::MapAccess::next_value::<
                                        _serde::de::IgnoredAny,
                                    >(&mut __map)
                                    {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    };
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("instance_name") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field1 = match __field1 {
                            _serde::__private::Some(__field1) => __field1,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("payload") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        _serde::__private::Ok(Device {
                            instance_name: __field0,
                            payload: __field1,
                        })
                    }
                }
                const FIELDS: &'static [&'static str] = &["instance_name", "payload"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "Device",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<Device>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for Device {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                let mut __serde_state = match _serde::Serializer::serialize_struct(
                    __serializer,
                    "Device",
                    false as usize + 1 + 1,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "instance_name",
                    &self.instance_name,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "payload",
                    &self.payload,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
    };
    /// Begin the migration process (source-side).
    ///
    ///This will attempt to upgrade the given HTTP request to a `propolis-migrate`
    /// connection and begin the migration in a separate task.
    pub async fn source_start(
        rqctx: Arc<RequestContext<Context>>,
        instance_id: Uuid,
        migration_id: Uuid,
    ) -> Result<Response<Body>, MigrateError> {
        let log = rqctx.log.new(::slog::OwnedKV((
            ::slog::SingleKV::from(("migrate_role", "source")),
            (
                ::slog::SingleKV::from(("migration_id", migration_id.to_string())),
                (),
            ),
        )));
        if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
            ::slog::Logger::log(&log, &{
                #[allow(dead_code)]
                static RS: ::slog::RecordStatic<'static> = {
                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                        file: "server/src/lib/migrate/mod.rs",
                        line: 255u32,
                        column: 5u32,
                        function: "",
                        module: "propolis_server::migrate",
                    };
                    ::slog::RecordStatic {
                        location: &LOC,
                        level: ::slog::Level::Info,
                        tag: "",
                    }
                };
                ::slog::Record::new(
                    &RS,
                    &::core::fmt::Arguments::new_v1(
                        &["Migration Source"],
                        &match () {
                            _args => [],
                        },
                    ),
                    ::slog::BorrowedKV(&()),
                )
            })
        };
        let mut context = rqctx.context().context.lock().await;
        let context = context
            .as_mut()
            .ok_or_else(|| MigrateError::InstanceNotInitialized)?;
        if instance_id != context.properties.id {
            return Err(MigrateError::UuidMismatch);
        }
        if !match context.instance.current_state() {
            State::Migrate(MigrateRole::Source, MigratePhase::Start) => true,
            _ => false,
        } {
            return Err(MigrateError::InvalidInstanceState);
        }
        let mut migrate_task = rqctx.context().migrate_task.lock().await;
        if migrate_task.is_some() {
            return Err(MigrateError::MigrationAlreadyInProgress);
        }
        let request = &mut *rqctx.request.lock().await;
        if !request
            .headers()
            .get(header::CONNECTION)
            .and_then(|hv| hv.to_str().ok())
            .map(|hv| hv.eq_ignore_ascii_case("upgrade"))
            .unwrap_or(false)
        {
            return Err(MigrateError::UpgradeExpected);
        }
        let src_protocol = MIGRATION_PROTOCOL_STR;
        let dst_protocol = request
            .headers()
            .get(header::UPGRADE)
            .ok_or_else(|| MigrateError::UpgradeExpected)
            .map(|hv| hv.to_str().ok())?
            .ok_or_else(|| MigrateError::incompatible(src_protocol, "<unknown>"))?;
        if !dst_protocol.eq_ignore_ascii_case(MIGRATION_PROTOCOL_STR) {
            if ::slog::Level::Error.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                ::slog::Logger::log(&log, &{
                    #[allow(dead_code)]
                    static RS: ::slog::RecordStatic<'static> = {
                        static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                            file: "server/src/lib/migrate/mod.rs",
                            line: 303u32,
                            column: 9u32,
                            function: "",
                            module: "propolis_server::migrate",
                        };
                        ::slog::RecordStatic {
                            location: &LOC,
                            level: ::slog::Level::Error,
                            tag: "",
                        }
                    };
                    ::slog::Record::new(
                        &RS,
                        &::core::fmt::Arguments::new_v1(
                            &[
                                "incompatible with destination instance provided protocol (",
                                ")",
                            ],
                            &match (&dst_protocol,) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ),
                        ::slog::BorrowedKV(&()),
                    )
                })
            };
            return Err(MigrateError::incompatible(src_protocol, dst_protocol));
        }
        let upgrade = hyper::upgrade::on(request);
        let migrate_context = Arc::new(MigrateContext::new(
            migration_id,
            context.instance.clone(),
            log.clone(),
        ));
        let mctx = migrate_context.clone();
        let task = tokio::spawn(async move {
            let conn = match upgrade.await {
                Ok(upgraded) => upgraded,
                Err(e) => {
                    if ::slog::Level::Error.as_usize()
                        <= ::slog::__slog_static_max_level().as_usize()
                    {
                        ::slog::Logger::log(&log, &{
                            #[allow(dead_code)]
                            static RS: ::slog::RecordStatic<'static> = {
                                static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                    file: "server/src/lib/migrate/mod.rs",
                                    line: 329u32,
                                    column: 17u32,
                                    function: "",
                                    module: "propolis_server::migrate",
                                };
                                ::slog::RecordStatic {
                                    location: &LOC,
                                    level: ::slog::Level::Error,
                                    tag: "",
                                }
                            };
                            ::slog::Record::new(
                                &RS,
                                &::core::fmt::Arguments::new_v1(
                                    &["Migrate Task Failed: "],
                                    &match (&e,) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Display::fmt,
                                        )],
                                    },
                                ),
                                ::slog::BorrowedKV(&()),
                            )
                        })
                    };
                    return;
                }
            };
            if let Err(e) = source::migrate(mctx, conn).await {
                if ::slog::Level::Error.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&log, &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/mod.rs",
                                line: 336u32,
                                column: 13u32,
                                function: "",
                                module: "propolis_server::migrate",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Error,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["Migrate Task Failed: "],
                                &match (&e,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Display::fmt,
                                    )],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                return;
            }
        });
        *migrate_task = Some(MigrateTask {
            task,
            context: migrate_context,
        });
        Ok(Response::builder()
            .status(StatusCode::SWITCHING_PROTOCOLS)
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, src_protocol)
            .body(Body::empty())
            .unwrap())
    }
    /// Initiate a migration to the given source instance.
    ///
    /// This will attempt to send an HTTP request, along with a request to upgrade
    /// it to a `propolis-migrate` connection, to the given source instance. Once
    /// we've successfully established the connection, we can begin the migration
    /// process (destination-side).
    pub async fn dest_initiate(
        rqctx: Arc<RequestContext<Context>>,
        instance_id: Uuid,
        migrate_info: api::InstanceMigrateInitiateRequest,
    ) -> Result<api::InstanceMigrateInitiateResponse, MigrateError> {
        let migration_id = migrate_info.migration_id;
        let log = rqctx.log.new(::slog::OwnedKV((
            ::slog::SingleKV::from(("migrate_src_addr", migrate_info.src_addr.clone())),
            (
                ::slog::SingleKV::from(("migrate_role", "destination")),
                (
                    ::slog::SingleKV::from(("migration_id", migration_id.to_string())),
                    (),
                ),
            ),
        )));
        if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
            ::slog::Logger::log(&log, &{
                #[allow(dead_code)]
                static RS: ::slog::RecordStatic<'static> = {
                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                        file: "server/src/lib/migrate/mod.rs",
                        line: 373u32,
                        column: 5u32,
                        function: "",
                        module: "propolis_server::migrate",
                    };
                    ::slog::RecordStatic {
                        location: &LOC,
                        level: ::slog::Level::Info,
                        tag: "",
                    }
                };
                ::slog::Record::new(
                    &RS,
                    &::core::fmt::Arguments::new_v1(
                        &["Migration Destination"],
                        &match () {
                            _args => [],
                        },
                    ),
                    ::slog::BorrowedKV(&()),
                )
            })
        };
        let mut context = rqctx.context().context.lock().await;
        let context = context
            .as_mut()
            .ok_or_else(|| MigrateError::InstanceNotInitialized)?;
        if instance_id != context.properties.id {
            return Err(MigrateError::UuidMismatch);
        }
        let mut migrate_task = rqctx.context().migrate_task.lock().await;
        if !migrate_task.is_none() {
            ::core::panicking::panic("assertion failed: migrate_task.is_none()")
        };
        let src_migrate_url = {
            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                &["http://", "/instances/", "/migrate/start"],
                &match (&migrate_info.src_addr, &migrate_info.src_uuid) {
                    _args => [
                        ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                        ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                    ],
                },
            ));
            res
        };
        if ::slog::Level::Info.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
            ::slog::Logger::log(&log, &{
                #[allow(dead_code)]
                static RS: ::slog::RecordStatic<'static> = {
                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                        file: "server/src/lib/migrate/mod.rs",
                        line: 394u32,
                        column: 5u32,
                        function: "",
                        module: "propolis_server::migrate",
                    };
                    ::slog::RecordStatic {
                        location: &LOC,
                        level: ::slog::Level::Info,
                        tag: "",
                    }
                };
                ::slog::Record::new(
                    &RS,
                    &::core::fmt::Arguments::new_v1(
                        &["Begin migration"],
                        &match () {
                            _args => [],
                        },
                    ),
                    ::slog::BorrowedKV(&(
                        ::slog::SingleKV::from(("src_migrate_url", &src_migrate_url)),
                        (),
                    )),
                )
            })
        };
        let body = Body::from(
            serde_json::to_string(&api::InstanceMigrateStartRequest { migration_id }).unwrap(),
        );
        let dst_protocol = MIGRATION_PROTOCOL_STR;
        let req = hyper::Request::builder()
            .method(Method::PUT)
            .uri(src_migrate_url)
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, dst_protocol)
            .body(body)
            .unwrap();
        let res = hyper::Client::new().request(req).await?;
        if res.status() != StatusCode::SWITCHING_PROTOCOLS {
            if ::slog::Level::Error.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                ::slog::Logger::log(&log, &{
                    #[allow(dead_code)]
                    static RS: ::slog::RecordStatic<'static> = {
                        static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                            file: "server/src/lib/migrate/mod.rs",
                            line: 417u32,
                            column: 9u32,
                            function: "",
                            module: "propolis_server::migrate",
                        };
                        ::slog::RecordStatic {
                            location: &LOC,
                            level: ::slog::Level::Error,
                            tag: "",
                        }
                    };
                    ::slog::Record::new(
                        &RS,
                        &::core::fmt::Arguments::new_v1(
                            &["source instance failed to switch protocols: "],
                            &match (&res.status(),) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ),
                        ::slog::BorrowedKV(&()),
                    )
                })
            };
            return Err(MigrateError::Initiate);
        }
        let src_protocol = res
            .headers()
            .get(header::UPGRADE)
            .ok_or_else(|| MigrateError::UpgradeExpected)
            .map(|hv| hv.to_str().ok())?
            .ok_or_else(|| MigrateError::incompatible("<unknown>", dst_protocol))?;
        if !src_protocol.eq_ignore_ascii_case(dst_protocol) {
            if ::slog::Level::Error.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                ::slog::Logger::log(&log, &{
                    #[allow(dead_code)]
                    static RS: ::slog::RecordStatic<'static> = {
                        static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                            file: "server/src/lib/migrate/mod.rs",
                            line: 433u32,
                            column: 9u32,
                            function: "",
                            module: "propolis_server::migrate",
                        };
                        ::slog::RecordStatic {
                            location: &LOC,
                            level: ::slog::Level::Error,
                            tag: "",
                        }
                    };
                    ::slog::Record::new(
                        &RS,
                        &::core::fmt::Arguments::new_v1(
                            &["incompatible with source instance provided protocol (", ")"],
                            &match (&src_protocol,) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ),
                        ::slog::BorrowedKV(&()),
                    )
                })
            };
            return Err(MigrateError::incompatible(src_protocol, dst_protocol));
        }
        let conn = hyper::upgrade::on(res).await?;
        let migrate_context = Arc::new(MigrateContext::new(
            migration_id,
            context.instance.clone(),
            log.clone(),
        ));
        let mctx = migrate_context.clone();
        let task = tokio::spawn(async move {
            if let Err(e) = destination::migrate(mctx, conn).await {
                if ::slog::Level::Error.as_usize() <= ::slog::__slog_static_max_level().as_usize() {
                    ::slog::Logger::log(&log, &{
                        #[allow(dead_code)]
                        static RS: ::slog::RecordStatic<'static> = {
                            static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                file: "server/src/lib/migrate/mod.rs",
                                line: 454u32,
                                column: 13u32,
                                function: "",
                                module: "propolis_server::migrate",
                            };
                            ::slog::RecordStatic {
                                location: &LOC,
                                level: ::slog::Level::Error,
                                tag: "",
                            }
                        };
                        ::slog::Record::new(
                            &RS,
                            &::core::fmt::Arguments::new_v1(
                                &["Migrate Task Failed: "],
                                &match (&e,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Display::fmt,
                                    )],
                                },
                            ),
                            ::slog::BorrowedKV(&()),
                        )
                    })
                };
                return;
            }
        });
        *migrate_task = Some(MigrateTask {
            task,
            context: migrate_context,
        });
        Ok(api::InstanceMigrateInitiateResponse { migration_id })
    }
    /// Return the current status of an ongoing migration
    pub async fn migrate_status(
        rqctx: Arc<RequestContext<Context>>,
        migration_id: Uuid,
    ) -> Result<api::InstanceMigrateStatusResponse, MigrateError> {
        let migrate_task = rqctx.context().migrate_task.lock().await;
        let migrate_task = migrate_task
            .as_ref()
            .ok_or_else(|| MigrateError::NoMigrationInProgress)?;
        if migration_id != migrate_task.context.migration_id {
            return Err(MigrateError::UuidMismatch);
        }
        Ok(api::InstanceMigrateStatusResponse {
            state: migrate_task.context.get_state().await,
        })
    }
    struct PageIter<'a> {
        start: u64,
        current: u64,
        end: u64,
        bits: &'a [u8],
    }
    impl<'a> PageIter<'a> {
        pub fn new(start: u64, end: u64, bits: &'a [u8]) -> PageIter<'a> {
            let current = start;
            PageIter {
                start,
                current,
                end,
                bits,
            }
        }
    }
    impl<'a> Iterator for PageIter<'a> {
        type Item = u64;
        fn next(&mut self) -> Option<Self::Item> {
            while self.current < self.end {
                let addr = self.current;
                self.current += 4096;
                let page_offset = ((addr - self.start) / 4096) as usize;
                let b = self.bits[page_offset / 8];
                if b.get_bit(page_offset % 8) {
                    return Some(addr);
                }
            }
            None
        }
    }
}
mod serial {
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::time::Duration;
    use propolis::chardev::{pollers, Sink, Source};
    use propolis::dispatch::AsyncCtx;
    /// Represents a serial connection into the VM.
    pub struct Serial<Device: Sink + Source> {
        uart: Arc<Device>,
        sink_poller: Arc<pollers::SinkBuffer>,
        source_poller: Arc<pollers::SourceBuffer>,
    }
    impl<Device: Sink + Source> Serial<Device> {
        /// Creates a new buffered serial connection on top of `uart.`
        ///
        /// Creation of this object disables "autodiscard", and destruction
        /// of the object re-enables "autodiscard" mode.
        ///
        /// # Arguments
        ///
        /// * `uart` - The device which data will be read from / written to.
        /// * `sink_size` - A lower bound on the size of the writeback buffer.
        /// * `source_size` - A lower bound on the size of the read buffer.
        pub fn new(
            uart: Arc<Device>,
            sink_size: NonZeroUsize,
            source_size: NonZeroUsize,
        ) -> Serial<Device> {
            let sink_poller = pollers::SinkBuffer::new(sink_size);
            let source_poller = pollers::SourceBuffer::new(pollers::Params {
                buf_size: source_size,
                poll_interval: Duration::from_millis(10),
                poll_miss_thresh: 5,
            });
            sink_poller.attach(uart.as_ref());
            source_poller.attach(uart.as_ref());
            uart.set_autodiscard(false);
            Serial {
                uart,
                sink_poller,
                source_poller,
            }
        }
        pub async fn read_source(&self, buf: &mut [u8], actx: &AsyncCtx) -> Option<usize> {
            self.source_poller.read(buf, self.uart.as_ref(), actx).await
        }
        pub async fn write_sink(&self, buf: &[u8], actx: &AsyncCtx) -> Option<usize> {
            self.sink_poller.write(buf, self.uart.as_ref(), actx).await
        }
    }
    impl<Device: Sink + Source> Drop for Serial<Device> {
        fn drop(&mut self) {
            self.uart.set_autodiscard(true);
        }
    }
}
pub mod vnc {
    pub mod server {
        use slog::{error, info, o, Logger};
        pub struct RamFb {
            addr: u64,
            width: usize,
            height: usize,
        }
        impl RamFb {
            pub fn new(addr: u64, width: usize, height: usize) -> Self {
                Self {
                    addr,
                    width,
                    height,
                }
            }
        }
        enum Framebuffer {
            Uninitialized,
            Initialized(RamFb),
        }
        pub struct VncServer {
            fb: Framebuffer,
        }
        impl VncServer {
            pub fn new(port: u16, log: Logger) -> Self {
                ::core::panicking::panic("not implemented")
            }
            pub fn start(&self) {
                ::core::panicking::panic("not implemented")
            }
            pub fn initialize_fb(&mut self, fb: RamFb) {
                self.fb = Framebuffer::Initialized(fb);
            }
            pub fn shutdown(&self) {
                ::core::panicking::panic("not implemented")
            }
        }
    }
}
pub mod server {
    //! HTTP server callback functions.
    use anyhow::Result;
    use dropshot::{
        endpoint, ApiDescription, HttpError, HttpResponseCreated, HttpResponseOk,
        HttpResponseUpdatedNoContent, Path, RequestContext, TypedBody,
    };
    use futures::future::Fuse;
    use futures::{FutureExt, SinkExt, StreamExt};
    use hyper::upgrade::{self, Upgraded};
    use hyper::{header, Body, Response, StatusCode};
    use slog::{error, info, o, Logger};
    use std::borrow::Cow;
    use std::io::{Error, ErrorKind};
    use std::ops::Range;
    use std::sync::Arc;
    use thiserror::Error;
    use tokio::sync::{oneshot, watch, Mutex};
    use tokio::task::JoinHandle;
    use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
    use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};
    use tokio_tungstenite::tungstenite::{self, handshake, protocol::Role, Message};
    use tokio_tungstenite::WebSocketStream;
    use uuid::Uuid;
    use propolis::bhyve_api;
    use propolis::dispatch::AsyncCtx;
    use propolis::hw::pci;
    use propolis::hw::uart::LpcUart;
    use propolis::instance::Instance;
    use propolis_client::api;
    use crate::config::Config;
    use crate::initializer::{build_instance, MachineInitializer};
    use crate::migrate;
    use crate::serial::Serial;
    use crate::vnc::server::VncServer;
    /// Errors which may occur during the course of a serial connection
    enum SerialTaskError {
        #[error("Cannot upgrade HTTP request to WebSockets: {0}")]
        Upgrade(#[from] hyper::Error),
        #[error("WebSocket Error: {0}")]
        WebSocket(#[from] tungstenite::Error),
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),
    }
    #[allow(unused_qualifications)]
    impl std::error::Error for SerialTaskError {
        fn source(&self) -> std::option::Option<&(dyn std::error::Error + 'static)> {
            use thiserror::private::AsDynError;
            #[allow(deprecated)]
            match self {
                SerialTaskError::Upgrade { 0: source, .. } => {
                    std::option::Option::Some(source.as_dyn_error())
                }
                SerialTaskError::WebSocket { 0: source, .. } => {
                    std::option::Option::Some(source.as_dyn_error())
                }
                SerialTaskError::Io { 0: source, .. } => {
                    std::option::Option::Some(source.as_dyn_error())
                }
            }
        }
    }
    #[allow(unused_qualifications)]
    impl std::fmt::Display for SerialTaskError {
        fn fmt(&self, __formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            #[allow(unused_imports)]
            use thiserror::private::{DisplayAsDisplay, PathAsDisplay};
            #[allow(unused_variables, deprecated, clippy::used_underscore_binding)]
            match self {
                SerialTaskError::Upgrade(_0) => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["Cannot upgrade HTTP request to WebSockets: "],
                        &match (&_0.as_display(),) {
                            _args => [::core::fmt::ArgumentV1::new(
                                _args.0,
                                ::core::fmt::Display::fmt,
                            )],
                        },
                    ))
                }
                SerialTaskError::WebSocket(_0) => {
                    __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                        &["WebSocket Error: "],
                        &match (&_0.as_display(),) {
                            _args => [::core::fmt::ArgumentV1::new(
                                _args.0,
                                ::core::fmt::Display::fmt,
                            )],
                        },
                    ))
                }
                SerialTaskError::Io(_0) => __formatter.write_fmt(::core::fmt::Arguments::new_v1(
                    &["IO error: "],
                    &match (&_0.as_display(),) {
                        _args => [::core::fmt::ArgumentV1::new(
                            _args.0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                )),
            }
        }
    }
    #[allow(unused_qualifications)]
    impl std::convert::From<hyper::Error> for SerialTaskError {
        #[allow(deprecated)]
        fn from(source: hyper::Error) -> Self {
            SerialTaskError::Upgrade { 0: source }
        }
    }
    #[allow(unused_qualifications)]
    impl std::convert::From<tungstenite::Error> for SerialTaskError {
        #[allow(deprecated)]
        fn from(source: tungstenite::Error) -> Self {
            SerialTaskError::WebSocket { 0: source }
        }
    }
    #[allow(unused_qualifications)]
    impl std::convert::From<std::io::Error> for SerialTaskError {
        #[allow(deprecated)]
        fn from(source: std::io::Error) -> Self {
            SerialTaskError::Io { 0: source }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for SerialTaskError {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match (&*self,) {
                (&SerialTaskError::Upgrade(ref __self_0),) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "Upgrade");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&SerialTaskError::WebSocket(ref __self_0),) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "WebSocket");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
                (&SerialTaskError::Io(ref __self_0),) => {
                    let debug_trait_builder = &mut ::core::fmt::Formatter::debug_tuple(f, "Io");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
            }
        }
    }
    struct SerialTask {
        /// Handle to attached serial session
        task: JoinHandle<()>,
        /// Oneshot channel used to detach an attached serial session
        detach_ch: oneshot::Sender<()>,
    }
    impl SerialTask {
        /// Is the serial task still attached
        fn is_attached(&self) -> bool {
            !self.detach_ch.is_closed()
        }
    }
    struct StateChange {
        gen: u64,
        state: propolis::instance::State,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for StateChange {
        #[inline]
        fn clone(&self) -> StateChange {
            match *self {
                StateChange {
                    gen: ref __self_0_0,
                    state: ref __self_0_1,
                } => StateChange {
                    gen: ::core::clone::Clone::clone(&(*__self_0_0)),
                    state: ::core::clone::Clone::clone(&(*__self_0_1)),
                },
            }
        }
    }
    pub(crate) struct InstanceContext {
        pub instance: Arc<Instance>,
        pub properties: api::InstanceProperties,
        serial: Option<Arc<Serial<LpcUart>>>,
        state_watcher: watch::Receiver<StateChange>,
        serial_task: Option<SerialTask>,
    }
    /// Contextual information accessible from HTTP callbacks.
    pub struct Context {
        pub(crate) context: Mutex<Option<InstanceContext>>,
        pub(crate) migrate_task: Mutex<Option<migrate::MigrateTask>>,
        config: Config,
        log: Logger,
        vnc_server: VncServer,
    }
    impl Context {
        /// Creates a new server context object.
        pub fn new(config: Config, vnc_server: VncServer, log: Logger) -> Self {
            Context {
                context: Mutex::new(None),
                migrate_task: Mutex::new(None),
                config,
                log,
                vnc_server,
            }
        }
    }
    fn api_to_propolis_state(state: api::InstanceStateRequested) -> propolis::instance::ReqState {
        use api::InstanceStateRequested as ApiState;
        use propolis::instance::ReqState as PropolisState;
        match state {
            ApiState::Run => PropolisState::Run,
            ApiState::Stop => PropolisState::Halt,
            ApiState::Reboot => PropolisState::Reset,
            ApiState::MigrateStart => PropolisState::StartMigrate,
        }
    }
    fn propolis_to_api_state(state: propolis::instance::State) -> api::InstanceState {
        use api::InstanceState as ApiState;
        use propolis::instance::State as PropolisState;
        match state {
            PropolisState::Initialize => ApiState::Creating,
            PropolisState::Boot => ApiState::Starting,
            PropolisState::Run => ApiState::Running,
            PropolisState::Quiesce => ApiState::Stopping,
            PropolisState::Migrate(_, _) => ApiState::Migrating,
            PropolisState::Halt => ApiState::Stopped,
            PropolisState::Reset => ApiState::Rebooting,
            PropolisState::Destroy => ApiState::Destroyed,
        }
    }
    enum SlotType {
        NIC,
        Disk,
        CloudInit,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for SlotType {
        #[inline]
        fn clone(&self) -> SlotType {
            {
                *self
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for SlotType {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for SlotType {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match (&*self,) {
                (&SlotType::NIC,) => ::core::fmt::Formatter::write_str(f, "NIC"),
                (&SlotType::Disk,) => ::core::fmt::Formatter::write_str(f, "Disk"),
                (&SlotType::CloudInit,) => ::core::fmt::Formatter::write_str(f, "CloudInit"),
            }
        }
    }
    fn slot_to_bdf(slot: api::Slot, ty: SlotType) -> Result<pci::Bdf> {
        match ty {
            SlotType::NIC if slot.0 <= 7 => Ok(pci::Bdf::new(0, slot.0 + 0x8, 0).unwrap()),
            SlotType::Disk if slot.0 <= 7 => Ok(pci::Bdf::new(0, slot.0 + 0x10, 0).unwrap()),
            SlotType::CloudInit if slot.0 == 0 => Ok(pci::Bdf::new(0, slot.0 + 0x18, 0).unwrap()),
            _ => Err(::anyhow::Error::msg({
                let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                    &["PCI Slot ", " has no translation to BDF for type "],
                    &match (&slot.0, &ty) {
                        _args => [
                            ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                            ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Debug::fmt),
                        ],
                    },
                ));
                res
            })),
        }
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstancePathParams>>();
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<TypedBody<api::InstanceEnsureRequest>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse(
            <Result<HttpResponseCreated<api::InstanceEnsureResponse>, HttpError> as ResultTrait>::T,
        );
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type::<
            <Result<HttpResponseCreated<api::InstanceEnsureResponse>, HttpError> as ResultTrait>::E,
        >();
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_ensure
    struct instance_ensure {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_ensure
    const instance_ensure: instance_ensure = instance_ensure {};
    impl From<instance_ensure>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_ensure) -> Self {
            async fn instance_ensure(
                rqctx: Arc<RequestContext<Context>>,
                path_params: Path<api::InstancePathParams>,
                request: TypedBody<api::InstanceEnsureRequest>,
            ) -> Result<HttpResponseCreated<api::InstanceEnsureResponse>, HttpError> {
                let server_context = rqctx.context();
                let request = request.into_inner();
                let instance_id = path_params.into_inner().instance_id;
                let (properties, nics, disks, cloud_init_bytes) = (
                    request.properties,
                    request.nics,
                    request.disks,
                    request.cloud_init_bytes,
                );
                if instance_id != properties.id {
                    return Err(HttpError::for_internal_error(
                        "UUID mismatch (path did not match struct)".to_string(),
                    ));
                }
                let mut context = server_context.context.lock().await;
                if let Some(ctx) = *context {
                    if ctx.properties.id != properties.id {
                        return Err(HttpError::for_internal_error({
                            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                &["Server already initialized with ID "],
                                &match (&ctx.properties.id,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Display::fmt,
                                    )],
                                },
                            ));
                            res
                        }));
                    }
                    if ctx.properties != properties {
                        return Err(HttpError::for_internal_error(
                            "Cannot update running server".to_string(),
                        ));
                    }
                    return Ok(HttpResponseCreated(api::InstanceEnsureResponse {
                        migrate: None,
                    }));
                }
                const MB: usize = 1024 * 1024;
                const GB: usize = 1024 * 1024 * 1024;
                let memsize = properties.memory as usize * MB;
                let lowmem = memsize.min(3 * GB);
                let highmem = memsize.saturating_sub(3 * GB);
                let vmm_log = server_context.log.new(::slog::OwnedKV((
                    ::slog::SingleKV::from(("component", "vmm")),
                    (),
                )));
                let instance = build_instance(
                    &properties.id.to_string(),
                    properties.vcpus,
                    lowmem,
                    highmem,
                    vmm_log,
                )
                .map_err(|err| {
                    HttpError::for_internal_error({
                        let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                            &["Cannot build instance: "],
                            &match (&err,) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Display::fmt,
                                )],
                            },
                        ));
                        res
                    })
                })?;
                let (tx, rx) = watch::channel(StateChange {
                    gen: 0,
                    state: propolis::instance::State::Initialize,
                });
                instance.on_transition(Box::new(move |next_state, _inv, _ctx| {
                    let last = (*tx.borrow()).clone();
                    let _ = tx.send(StateChange {
                        gen: last.gen + 1,
                        state: next_state,
                    });
                }));
                let mut com1 = None;
                let mut framebuffer = None;
                instance
                    .initialize(|machine, mctx, disp, inv| {
                        let init =
                            MachineInitializer::new(rqctx.log.clone(), machine, mctx, disp, inv);
                        init.initialize_rom(server_context.config.get_bootrom())?;
                        init.initialize_kernel_devs(lowmem, highmem)?;
                        let chipset = init.initialize_chipset()?;
                        com1 = Some(Arc::new(init.initialize_uart(&chipset)?));
                        init.initialize_ps2(&chipset)?;
                        init.initialize_qemu_debug_port()?;
                        for nic in &nics {
                            if ::slog::Level::Info.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&rqctx.log, &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/server.rs",
                                                line: 298u32,
                                                column: 17u32,
                                                function: "",
                                                module: "propolis_server::server",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Info,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1_formatted(
                                            &["Creating NIC: "],
                                            &match (&nic,) {
                                                _args => [::core::fmt::ArgumentV1::new(
                                                    _args.0,
                                                    ::core::fmt::Debug::fmt,
                                                )],
                                            },
                                            &[::core::fmt::rt::v1::Argument {
                                                position: 0usize,
                                                format: ::core::fmt::rt::v1::FormatSpec {
                                                    fill: ' ',
                                                    align: ::core::fmt::rt::v1::Alignment::Unknown,
                                                    flags: 4u32,
                                                    precision: ::core::fmt::rt::v1::Count::Implied,
                                                    width: ::core::fmt::rt::v1::Count::Implied,
                                                },
                                            }],
                                            unsafe { ::core::fmt::UnsafeArg::new() },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                            let bdf = slot_to_bdf(nic.slot, SlotType::NIC).map_err(|e| {
                                Error::new(ErrorKind::InvalidData, {
                                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                        &["Cannot parse vnic PCI: "],
                                        &match (&e,) {
                                            _args => [::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Display::fmt,
                                            )],
                                        },
                                    ));
                                    res
                                })
                            })?;
                            init.initialize_vnic(&chipset, &nic.name, bdf)?;
                        }
                        for disk in &disks {
                            if ::slog::Level::Info.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&rqctx.log, &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/server.rs",
                                                line: 310u32,
                                                column: 17u32,
                                                function: "",
                                                module: "propolis_server::server",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Info,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1_formatted(
                                            &["Creating Disk: "],
                                            &match (&disk,) {
                                                _args => [::core::fmt::ArgumentV1::new(
                                                    _args.0,
                                                    ::core::fmt::Debug::fmt,
                                                )],
                                            },
                                            &[::core::fmt::rt::v1::Argument {
                                                position: 0usize,
                                                format: ::core::fmt::rt::v1::FormatSpec {
                                                    fill: ' ',
                                                    align: ::core::fmt::rt::v1::Alignment::Unknown,
                                                    flags: 4u32,
                                                    precision: ::core::fmt::rt::v1::Count::Implied,
                                                    width: ::core::fmt::rt::v1::Count::Implied,
                                                },
                                            }],
                                            unsafe { ::core::fmt::UnsafeArg::new() },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                            let bdf = slot_to_bdf(disk.slot, SlotType::Disk).map_err(|e| {
                                Error::new(ErrorKind::InvalidData, {
                                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                        &["Cannot parse disk PCI: "],
                                        &match (&e,) {
                                            _args => [::core::fmt::ArgumentV1::new(
                                                _args.0,
                                                ::core::fmt::Display::fmt,
                                            )],
                                        },
                                    ));
                                    res
                                })
                            })?;
                            init.initialize_crucible(&chipset, disk, bdf)?;
                            if ::slog::Level::Info.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&rqctx.log, &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/server.rs",
                                                line: 320u32,
                                                column: 17u32,
                                                function: "",
                                                module: "propolis_server::server",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Info,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["Disk ", " created successfully"],
                                            &match (&disk.name,) {
                                                _args => [::core::fmt::ArgumentV1::new(
                                                    _args.0,
                                                    ::core::fmt::Display::fmt,
                                                )],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                        }
                        if let Some(cloud_init_bytes) = &cloud_init_bytes {
                            if ::slog::Level::Info.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&rqctx.log, &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/server.rs",
                                                line: 324u32,
                                                column: 17u32,
                                                function: "",
                                                module: "propolis_server::server",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Info,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["Creating cloud-init disk"],
                                            &match () {
                                                _args => [],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                            let bdf = slot_to_bdf(api::Slot(0), SlotType::CloudInit)
                                .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
                            let bytes = base64::decode(&cloud_init_bytes)
                                .map_err(|e| Error::new(ErrorKind::InvalidInput, e.to_string()))?;
                            init.initialize_in_memory_virtio_from_bytes(
                                &chipset, bytes, bdf, true,
                            )?;
                            if ::slog::Level::Info.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&rqctx.log, &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/server.rs",
                                                line: 338u32,
                                                column: 17u32,
                                                function: "",
                                                module: "propolis_server::server",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Info,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["cloud-init disk created"],
                                            &match () {
                                                _args => [],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                        }
                        for (devname, dev) in server_context.config.devs() {
                            let driver = &dev.driver as &str;
                            match driver {
                                "pci-virtio-block" => {
                                    let block_dev_name = dev
                                        .options
                                        .get("block_dev")
                                        .ok_or_else(|| {
                                            Error::new(ErrorKind::InvalidData, {
                                                let res = ::alloc::fmt::format(
                                                    ::core::fmt::Arguments::new_v1(
                                                        &["no block_dev key for ", "!"],
                                                        &match (&devname,) {
                                                            _args => {
                                                                [::core::fmt::ArgumentV1::new(
                                                                    _args.0,
                                                                    ::core::fmt::Display::fmt,
                                                                )]
                                                            }
                                                        },
                                                    ),
                                                );
                                                res
                                            })
                                        })?
                                        .as_str()
                                        .ok_or_else(|| {
                                            Error::new(ErrorKind::InvalidData, {
                                                let res = ::alloc::fmt::format(
                                                    ::core::fmt::Arguments::new_v1(
                                                        &["as_str() failed for ", "\'s block_dev!"],
                                                        &match (&devname,) {
                                                            _args => {
                                                                [::core::fmt::ArgumentV1::new(
                                                                    _args.0,
                                                                    ::core::fmt::Display::fmt,
                                                                )]
                                                            }
                                                        },
                                                    ),
                                                );
                                                res
                                            })
                                        })?;
                                    let (backend, creg) = server_context
                                        .config
                                        .create_block_backend(block_dev_name, &disp)
                                        .map_err(|e| {
                                            Error::new(ErrorKind::InvalidData, {
                                                let res = ::alloc::fmt::format(
                                                    ::core::fmt::Arguments::new_v1(
                                                        &["ParseError: "],
                                                        &match (&e,) {
                                                            _args => {
                                                                [::core::fmt::ArgumentV1::new(
                                                                    _args.0,
                                                                    ::core::fmt::Debug::fmt,
                                                                )]
                                                            }
                                                        },
                                                    ),
                                                );
                                                res
                                            })
                                        })?;
                                    let bdf: pci::Bdf = dev.get("pci-path").ok_or_else(|| {
                                        Error::new(ErrorKind::InvalidData, "Cannot parse disk PCI")
                                    })?;
                                    init.initialize_virtio_block(&chipset, bdf, backend, creg)?;
                                }
                                "pci-nvme" => {
                                    let block_dev_name = dev
                                        .options
                                        .get("block_dev")
                                        .ok_or_else(|| {
                                            Error::new(ErrorKind::InvalidData, {
                                                let res = ::alloc::fmt::format(
                                                    ::core::fmt::Arguments::new_v1(
                                                        &["no block_dev key for ", "!"],
                                                        &match (&devname,) {
                                                            _args => {
                                                                [::core::fmt::ArgumentV1::new(
                                                                    _args.0,
                                                                    ::core::fmt::Display::fmt,
                                                                )]
                                                            }
                                                        },
                                                    ),
                                                );
                                                res
                                            })
                                        })?
                                        .as_str()
                                        .ok_or_else(|| {
                                            Error::new(ErrorKind::InvalidData, {
                                                let res = ::alloc::fmt::format(
                                                    ::core::fmt::Arguments::new_v1(
                                                        &["as_str() failed for ", "\'s block_dev!"],
                                                        &match (&devname,) {
                                                            _args => {
                                                                [::core::fmt::ArgumentV1::new(
                                                                    _args.0,
                                                                    ::core::fmt::Display::fmt,
                                                                )]
                                                            }
                                                        },
                                                    ),
                                                );
                                                res
                                            })
                                        })?;
                                    let (backend, creg) = server_context
                                        .config
                                        .create_block_backend(block_dev_name, &disp)
                                        .map_err(|e| {
                                            Error::new(ErrorKind::InvalidData, {
                                                let res = ::alloc::fmt::format(
                                                    ::core::fmt::Arguments::new_v1(
                                                        &["ParseError: "],
                                                        &match (&e,) {
                                                            _args => {
                                                                [::core::fmt::ArgumentV1::new(
                                                                    _args.0,
                                                                    ::core::fmt::Debug::fmt,
                                                                )]
                                                            }
                                                        },
                                                    ),
                                                );
                                                res
                                            })
                                        })?;
                                    let bdf: pci::Bdf = dev.get("pci-path").ok_or_else(|| {
                                        Error::new(ErrorKind::InvalidData, "Cannot parse disk PCI")
                                    })?;
                                    init.initialize_nvme_block(
                                        &chipset,
                                        bdf,
                                        block_dev_name.to_string(),
                                        backend,
                                        creg,
                                    )?;
                                }
                                "pci-virtio-viona" => {
                                    let name = dev.get_string("vnic").ok_or_else(|| {
                                        Error::new(ErrorKind::InvalidData, "Cannot parse vnic name")
                                    })?;
                                    let bdf: pci::Bdf = dev.get("pci-path").ok_or_else(|| {
                                        Error::new(ErrorKind::InvalidData, "Cannot parse vnic PCI")
                                    })?;
                                    init.initialize_vnic(&chipset, name, bdf)?;
                                }
                                _ => {
                                    return Err(Error::new(ErrorKind::InvalidData, {
                                        let res =
                                            ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                                &["Unknown driver in config: "],
                                                &match (&driver,) {
                                                    _args => [::core::fmt::ArgumentV1::new(
                                                        _args.0,
                                                        ::core::fmt::Display::fmt,
                                                    )],
                                                },
                                            ));
                                        res
                                    }));
                                }
                            }
                        }
                        framebuffer = Some(init.initialize_fwcfg(properties.vcpus)?);
                        init.initialize_cpus()?;
                        Ok(())
                    })
                    .map_err(|err| {
                        HttpError::for_internal_error({
                            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                &["Failed to initialize machine: "],
                                &match (&err,) {
                                    _args => [::core::fmt::ArgumentV1::new(
                                        _args.0,
                                        ::core::fmt::Display::fmt,
                                    )],
                                },
                            ));
                            res
                        })
                    })?;
                server_context
                    .vnc_server
                    .initialize_fb(framebuffer.unwrap());
                *context = Some(InstanceContext {
                    instance: instance.clone(),
                    properties,
                    serial: com1,
                    state_watcher: rx,
                    serial_task: None,
                });
                drop(context);
                let migrate = if let Some(migrate_request) = request.migrate {
                    let res = migrate::dest_initiate(rqctx, instance_id, migrate_request)
                        .await
                        .map_err(<_ as Into<HttpError>>::into)?;
                    Some(res)
                } else {
                    instance.on_transition(Box::new(
                        move |next_state, _inv, ctx| match next_state {
                            propolis::instance::State::Boot => {
                                for mut vcpu in ctx.mctx.vcpus() {
                                    vcpu.reboot_state().unwrap();
                                    vcpu.activate().unwrap();
                                    if vcpu.is_bsp() {
                                        vcpu.set_run_state(bhyve_api::VRS_RUN).unwrap();
                                        vcpu.set_reg(
                                            bhyve_api::vm_reg_name::VM_REG_GUEST_RIP,
                                            0xfff0,
                                        )
                                        .unwrap();
                                    }
                                }
                            }
                            _ => {}
                        },
                    ));
                    None
                };
                instance.print();
                Ok(HttpResponseCreated(api::InstanceEnsureResponse { migrate }))
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstancePathParams>,
                    arg2: TypedBody<api::InstanceEnsureRequest>,
                ) {
                    future_endpoint_must_be_send(instance_ensure(arg0, arg1, arg2));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_ensure".to_string(),
                instance_ensure,
                dropshot::Method::PUT,
                "/instances/{instance_id}",
            )
        }
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstanceNameParams>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse(<Result<HttpResponseOk<Uuid>, HttpError> as ResultTrait>::T);
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type::<<Result<HttpResponseOk<Uuid>, HttpError> as ResultTrait>::E>();
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_get_uuid
    struct instance_get_uuid {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_get_uuid
    const instance_get_uuid: instance_get_uuid = instance_get_uuid {};
    impl From<instance_get_uuid>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_get_uuid) -> Self {
            async fn instance_get_uuid(
                rqctx: Arc<RequestContext<Context>>,
                path_params: Path<api::InstanceNameParams>,
            ) -> Result<HttpResponseOk<Uuid>, HttpError> {
                let context = rqctx.context().context.lock().await;
                let context = context.as_ref().ok_or_else(|| {
                    HttpError::for_internal_error(
                        "Server not initialized (no instance)".to_string(),
                    )
                })?;
                if path_params.into_inner().instance_id != context.properties.name {
                    return Err(HttpError::for_internal_error(
                        "Instance name mismatch (path did not match struct)".to_string(),
                    ));
                }
                Ok(HttpResponseOk(context.properties.id))
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstanceNameParams>,
                ) {
                    future_endpoint_must_be_send(instance_get_uuid(arg0, arg1));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_get_uuid".to_string(),
                instance_get_uuid,
                dropshot::Method::GET,
                "/instances/{instance_id}/uuid",
            )
            .visible(false)
        }
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstancePathParams>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse(
            <Result<HttpResponseOk<api::InstanceGetResponse>, HttpError> as ResultTrait>::T,
        );
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type::<
            <Result<HttpResponseOk<api::InstanceGetResponse>, HttpError> as ResultTrait>::E,
        >();
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_get
    struct instance_get {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_get
    const instance_get: instance_get = instance_get {};
    impl From<instance_get>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_get) -> Self {
            async fn instance_get(
                rqctx: Arc<RequestContext<Context>>,
                path_params: Path<api::InstancePathParams>,
            ) -> Result<HttpResponseOk<api::InstanceGetResponse>, HttpError> {
                let context = rqctx.context().context.lock().await;
                let context = context.as_ref().ok_or_else(|| {
                    HttpError::for_internal_error(
                        "Server not initialized (no instance)".to_string(),
                    )
                })?;
                if path_params.into_inner().instance_id != context.properties.id {
                    return Err(HttpError::for_internal_error(
                        "UUID mismatch (path did not match struct)".to_string(),
                    ));
                }
                let instance_info = api::Instance {
                    properties: context.properties.clone(),
                    state: propolis_to_api_state(context.instance.current_state()),
                    disks: ::alloc::vec::Vec::new(),
                    nics: ::alloc::vec::Vec::new(),
                };
                Ok(HttpResponseOk(api::InstanceGetResponse {
                    instance: instance_info,
                }))
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstancePathParams>,
                ) {
                    future_endpoint_must_be_send(instance_get(arg0, arg1));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_get".to_string(),
                instance_get,
                dropshot::Method::GET,
                "/instances/{instance_id}",
            )
        }
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstancePathParams>>();
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<TypedBody<api::InstanceStateMonitorRequest>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse (< Result < HttpResponseOk < api :: InstanceStateMonitorResponse > , HttpError > as ResultTrait > :: T) ;
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type :: < < Result < HttpResponseOk < api :: InstanceStateMonitorResponse > , HttpError > as ResultTrait > :: E > () ;
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_state_monitor
    struct instance_state_monitor {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_state_monitor
    const instance_state_monitor: instance_state_monitor = instance_state_monitor {};
    impl From<instance_state_monitor>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_state_monitor) -> Self {
            async fn instance_state_monitor(
                rqctx: Arc<RequestContext<Context>>,
                path_params: Path<api::InstancePathParams>,
                request: TypedBody<api::InstanceStateMonitorRequest>,
            ) -> Result<HttpResponseOk<api::InstanceStateMonitorResponse>, HttpError> {
                let (mut state_watcher, gen) = {
                    let context = rqctx.context().context.lock().await;
                    let context = context.as_ref().ok_or_else(|| {
                        HttpError::for_internal_error(
                            "Server not initialized (no instance)".to_string(),
                        )
                    })?;
                    let path_params = path_params.into_inner();
                    if path_params.instance_id != context.properties.id {
                        return Err(HttpError::for_internal_error(
                            "UUID mismatch (path did not match struct)".to_string(),
                        ));
                    }
                    let gen = request.into_inner().gen;
                    let state_watcher = context.state_watcher.clone();
                    (state_watcher, gen)
                };
                loop {
                    let last = state_watcher.borrow().clone();
                    if gen <= last.gen {
                        let response = api::InstanceStateMonitorResponse {
                            gen: last.gen,
                            state: propolis_to_api_state(last.state),
                        };
                        return Ok(HttpResponseOk(response));
                    }
                    state_watcher.changed().await.unwrap();
                }
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstancePathParams>,
                    arg2: TypedBody<api::InstanceStateMonitorRequest>,
                ) {
                    future_endpoint_must_be_send(instance_state_monitor(arg0, arg1, arg2));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_state_monitor".to_string(),
                instance_state_monitor,
                dropshot::Method::GET,
                "/instances/{instance_id}/state-monitor",
            )
        }
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstancePathParams>>();
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<TypedBody<api::InstanceStateRequested>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse(
            <Result<HttpResponseUpdatedNoContent, HttpError> as ResultTrait>::T,
        );
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type::<
            <Result<HttpResponseUpdatedNoContent, HttpError> as ResultTrait>::E,
        >();
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_state_put
    struct instance_state_put {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_state_put
    const instance_state_put: instance_state_put = instance_state_put {};
    impl From<instance_state_put>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_state_put) -> Self {
            async fn instance_state_put(
                rqctx: Arc<RequestContext<Context>>,
                path_params: Path<api::InstancePathParams>,
                request: TypedBody<api::InstanceStateRequested>,
            ) -> Result<HttpResponseUpdatedNoContent, HttpError> {
                let context = rqctx.context().context.lock().await;
                let context = context.as_ref().ok_or_else(|| {
                    HttpError::for_internal_error(
                        "Server not initialized (no instance)".to_string(),
                    )
                })?;
                if path_params.into_inner().instance_id != context.properties.id {
                    return Err(HttpError::for_internal_error(
                        "UUID mismatch (path did not match struct)".to_string(),
                    ));
                }
                let state = api_to_propolis_state(request.into_inner());
                context.instance.set_target_state(state).map_err(|err| {
                    HttpError::for_internal_error({
                        let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                            &["Failed to set state: "],
                            &match (&err,) {
                                _args => [::core::fmt::ArgumentV1::new(
                                    _args.0,
                                    ::core::fmt::Debug::fmt,
                                )],
                            },
                        ));
                        res
                    })
                })?;
                Ok(HttpResponseUpdatedNoContent {})
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstancePathParams>,
                    arg2: TypedBody<api::InstanceStateRequested>,
                ) {
                    future_endpoint_must_be_send(instance_state_put(arg0, arg1, arg2));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_state_put".to_string(),
                instance_state_put,
                dropshot::Method::PUT,
                "/instances/{instance_id}/state",
            )
        }
    }
    async fn instance_serial_task(
        mut detach: oneshot::Receiver<()>,
        serial: Arc<Serial<LpcUart>>,
        ws_stream: WebSocketStream<Upgraded>,
        log: Logger,
        actx: &AsyncCtx,
    ) -> Result<(), SerialTaskError> {
        let mut output = [0u8; 1024];
        let mut cur_output: Option<Range<usize>> = None;
        let mut cur_input: Option<(Vec<u8>, usize)> = None;
        let (mut ws_sink, mut ws_stream) = ws_stream.split();
        loop {
            let (uart_read, ws_send) = match &cur_output {
                None => (
                    serial.read_source(&mut output, actx).fuse(),
                    Fuse::terminated(),
                ),
                Some(r) => (
                    Fuse::terminated(),
                    ws_sink.send(Message::binary(&output[r.clone()])).fuse(),
                ),
            };
            let (ws_recv, uart_write) = match &cur_input {
                None => (ws_stream.next().fuse(), Fuse::terminated()),
                Some((data, consumed)) => (
                    Fuse::terminated(),
                    serial.write_sink(&data[*consumed..], actx).fuse(),
                ),
            };
            {
                mod util {
                    pub(super) enum Out<_0, _1, _2, _3, _4> {
                        _0(_0),
                        _1(_1),
                        _2(_2),
                        _3(_3),
                        _4(_4),
                        Disabled,
                    }
                    pub(super) type Mask = u8;
                }
                use ::tokio::macros::support::Future;
                use ::tokio::macros::support::Pin;
                use ::tokio::macros::support::Poll::{Ready, Pending};
                const BRANCHES: u32 = 5;
                let mut disabled: util::Mask = Default::default();
                if !true {
                    let mask: util::Mask = 1 << 0;
                    disabled |= mask;
                }
                if !true {
                    let mask: util::Mask = 1 << 1;
                    disabled |= mask;
                }
                if !true {
                    let mask: util::Mask = 1 << 2;
                    disabled |= mask;
                }
                if !true {
                    let mask: util::Mask = 1 << 3;
                    disabled |= mask;
                }
                if !true {
                    let mask: util::Mask = 1 << 4;
                    disabled |= mask;
                }
                let mut output = {
                    let mut futures = (&mut detach, uart_write, ws_send, uart_read, ws_recv);
                    ::tokio::macros::support::poll_fn(|cx| {
                        let mut is_pending = false;
                        let start = 0;
                        for i in 0..BRANCHES {
                            let branch;
                            #[allow(clippy::modulo_one)]
                            {
                                branch = (start + i) % BRANCHES;
                            }
                            match branch {
                                #[allow(unreachable_code)]
                                0 => {
                                    let mask = 1 << branch;
                                    if disabled & mask == mask {
                                        continue;
                                    }
                                    let (fut, ..) = &mut futures;
                                    let mut fut = unsafe { Pin::new_unchecked(fut) };
                                    let out = match Future::poll(fut, cx) {
                                        Ready(out) => out,
                                        Pending => {
                                            is_pending = true;
                                            continue;
                                        }
                                    };
                                    disabled |= mask;
                                    #[allow(unused_variables)]
                                    #[allow(unused_mut)]
                                    match &out {
                                        _ => {}
                                        _ => continue,
                                    }
                                    return Ready(util::Out::_0(out));
                                }
                                #[allow(unreachable_code)]
                                1 => {
                                    let mask = 1 << branch;
                                    if disabled & mask == mask {
                                        continue;
                                    }
                                    let (_, fut, ..) = &mut futures;
                                    let mut fut = unsafe { Pin::new_unchecked(fut) };
                                    let out = match Future::poll(fut, cx) {
                                        Ready(out) => out,
                                        Pending => {
                                            is_pending = true;
                                            continue;
                                        }
                                    };
                                    disabled |= mask;
                                    #[allow(unused_variables)]
                                    #[allow(unused_mut)]
                                    match &out {
                                        written => {}
                                        _ => continue,
                                    }
                                    return Ready(util::Out::_1(out));
                                }
                                #[allow(unreachable_code)]
                                2 => {
                                    let mask = 1 << branch;
                                    if disabled & mask == mask {
                                        continue;
                                    }
                                    let (_, _, fut, ..) = &mut futures;
                                    let mut fut = unsafe { Pin::new_unchecked(fut) };
                                    let out = match Future::poll(fut, cx) {
                                        Ready(out) => out,
                                        Pending => {
                                            is_pending = true;
                                            continue;
                                        }
                                    };
                                    disabled |= mask;
                                    #[allow(unused_variables)]
                                    #[allow(unused_mut)]
                                    match &out {
                                        write_success => {}
                                        _ => continue,
                                    }
                                    return Ready(util::Out::_2(out));
                                }
                                #[allow(unreachable_code)]
                                3 => {
                                    let mask = 1 << branch;
                                    if disabled & mask == mask {
                                        continue;
                                    }
                                    let (_, _, _, fut, ..) = &mut futures;
                                    let mut fut = unsafe { Pin::new_unchecked(fut) };
                                    let out = match Future::poll(fut, cx) {
                                        Ready(out) => out,
                                        Pending => {
                                            is_pending = true;
                                            continue;
                                        }
                                    };
                                    disabled |= mask;
                                    #[allow(unused_variables)]
                                    #[allow(unused_mut)]
                                    match &out {
                                        nread => {}
                                        _ => continue,
                                    }
                                    return Ready(util::Out::_3(out));
                                }
                                #[allow(unreachable_code)]
                                4 => {
                                    let mask = 1 << branch;
                                    if disabled & mask == mask {
                                        continue;
                                    }
                                    let (_, _, _, _, fut, ..) = &mut futures;
                                    let mut fut = unsafe { Pin::new_unchecked(fut) };
                                    let out = match Future::poll(fut, cx) {
                                        Ready(out) => out,
                                        Pending => {
                                            is_pending = true;
                                            continue;
                                        }
                                    };
                                    disabled |= mask;
                                    #[allow(unused_variables)]
                                    #[allow(unused_mut)]
                                    match &out {
                                        msg => {}
                                        _ => continue,
                                    }
                                    return Ready(util::Out::_4(out));
                                }
                                _ => ::core::panicking::panic_fmt(::core::fmt::Arguments::new_v1(
                                    &["internal error: entered unreachable code: "],
                                    &match (
                                        &"reaching this means there probably is an off by one bug",
                                    ) {
                                        _args => [::core::fmt::ArgumentV1::new(
                                            _args.0,
                                            ::core::fmt::Display::fmt,
                                        )],
                                    },
                                )),
                            }
                        }
                        if is_pending {
                            Pending
                        } else {
                            Ready(util::Out::Disabled)
                        }
                    })
                    .await
                };
                match output {
                    util::Out::_0(_) => {
                        if ::slog::Level::Info.as_usize()
                            <= ::slog::__slog_static_max_level().as_usize()
                        {
                            ::slog::Logger::log(&log, &{
                                #[allow(dead_code)]
                                static RS: ::slog::RecordStatic<'static> = {
                                    static LOC: ::slog::RecordLocation = ::slog::RecordLocation {
                                        file: "server/src/lib/server.rs",
                                        line: 712u32,
                                        column: 17u32,
                                        function: "",
                                        module: "propolis_server::server",
                                    };
                                    ::slog::RecordStatic {
                                        location: &LOC,
                                        level: ::slog::Level::Info,
                                        tag: "",
                                    }
                                };
                                ::slog::Record::new(
                                    &RS,
                                    &::core::fmt::Arguments::new_v1(
                                        &["Detaching from serial console"],
                                        &match () {
                                            _args => [],
                                        },
                                    ),
                                    ::slog::BorrowedKV(&()),
                                )
                            })
                        };
                        let close = CloseFrame {
                            code: CloseCode::Policy,
                            reason: Cow::Borrowed("serial console was detached"),
                        };
                        ws_sink.send(Message::Close(Some(close))).await?;
                        break;
                    }
                    util::Out::_1(written) => match written {
                        Some(0) | None => break,
                        Some(n) => {
                            let (data, consumed) = cur_input.as_mut().unwrap();
                            *consumed += n;
                            if *consumed == data.len() {
                                cur_input = None;
                            }
                        }
                    },
                    util::Out::_2(write_success) => {
                        write_success?;
                        cur_output = None;
                    }
                    util::Out::_3(nread) => match nread {
                        Some(0) | None => break,
                        Some(n) => cur_output = Some(0..n),
                    },
                    util::Out::_4(msg) => match msg {
                        Some(Ok(Message::Binary(input))) => {
                            cur_input = Some((input, 0));
                        }
                        Some(Ok(Message::Close(..))) | None => break,
                        _ => continue,
                    },
                    util::Out::Disabled => ::std::rt::begin_panic(
                        "all branches are disabled and there is no else branch",
                    ),
                    _ => ::core::panicking::panic_fmt(::core::fmt::Arguments::new_v1(
                        &["internal error: entered unreachable code: "],
                        &match (&"failed to match bind",) {
                            _args => [::core::fmt::ArgumentV1::new(
                                _args.0,
                                ::core::fmt::Display::fmt,
                            )],
                        },
                    )),
                }
            }
        }
        Ok(())
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstancePathParams>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse(<Result<Response<Body>, HttpError> as ResultTrait>::T);
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type::<<Result<Response<Body>, HttpError> as ResultTrait>::E>();
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_serial
    struct instance_serial {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_serial
    const instance_serial: instance_serial = instance_serial {};
    impl From<instance_serial>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_serial) -> Self {
            async fn instance_serial(
                rqctx: Arc<RequestContext<Context>>,
                path_params: Path<api::InstancePathParams>,
            ) -> Result<Response<Body>, HttpError> {
                let mut context = rqctx.context().context.lock().await;
                let context = context.as_mut().ok_or_else(|| {
                    HttpError::for_internal_error(
                        "Server not initialized (no instance)".to_string(),
                    )
                })?;
                if path_params.into_inner().instance_id != context.properties.id {
                    return Err(HttpError::for_internal_error(
                        "UUID mismatch (path did not match struct)".to_string(),
                    ));
                }
                if context
                    .serial_task
                    .as_ref()
                    .map_or(false, |s| s.is_attached())
                {
                    return Err(HttpError::for_unavail(
                        None,
                        "serial console already attached".to_string(),
                    ));
                }
                let serial = context
                    .serial
                    .as_ref()
                    .ok_or_else(|| {
                        HttpError::for_internal_error(
                            "Instance present but serial not initialized".to_string(),
                        )
                    })?
                    .clone();
                let request = &mut *rqctx.request.lock().await;
                if !request
                    .headers()
                    .get(header::CONNECTION)
                    .and_then(|hv| hv.to_str().ok())
                    .map(|hv| {
                        hv.split(|c| c == ',' || c == ' ')
                            .any(|vs| vs.eq_ignore_ascii_case("upgrade"))
                    })
                    .unwrap_or(false)
                {
                    return Err(HttpError::for_bad_request(
                        None,
                        "expected connection upgrade".to_string(),
                    ));
                }
                if !request
                    .headers()
                    .get(header::UPGRADE)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| {
                        v.split(|c| c == ',' || c == ' ')
                            .any(|v| v.eq_ignore_ascii_case("websocket"))
                    })
                    .unwrap_or(false)
                {
                    return Err(HttpError::for_bad_request(
                        None,
                        "unexpected protocol for upgrade".to_string(),
                    ));
                }
                if request
                    .headers()
                    .get(header::SEC_WEBSOCKET_VERSION)
                    .map(|v| v.as_bytes())
                    != Some(b"13")
                {
                    return Err(HttpError::for_bad_request(
                        None,
                        "missing or invalid websocket version".to_string(),
                    ));
                }
                let accept_key = request
                    .headers()
                    .get(header::SEC_WEBSOCKET_KEY)
                    .map(|hv| hv.as_bytes())
                    .map(|key| handshake::derive_accept_key(key))
                    .ok_or_else(|| {
                        HttpError::for_bad_request(None, "missing websocket key".to_string())
                    })?;
                let (detach_ch, detach_recv) = oneshot::channel();
                let upgrade_fut = upgrade::on(request);
                let ws_log = rqctx.log.new(::slog::OwnedKV(()));
                let err_log = ws_log.clone();
                let actx = context.instance.disp.async_ctx();
                let task = tokio::spawn(async move {
                    let upgraded = match upgrade_fut.await {
                        Ok(u) => u,
                        Err(e) => {
                            if ::slog::Level::Error.as_usize()
                                <= ::slog::__slog_static_max_level().as_usize()
                            {
                                ::slog::Logger::log(&err_log, &{
                                    #[allow(dead_code)]
                                    static RS: ::slog::RecordStatic<'static> = {
                                        static LOC: ::slog::RecordLocation =
                                            ::slog::RecordLocation {
                                                file: "server/src/lib/server.rs",
                                                line: 866u32,
                                                column: 17u32,
                                                function: "",
                                                module: "propolis_server::server",
                                            };
                                        ::slog::RecordStatic {
                                            location: &LOC,
                                            level: ::slog::Level::Error,
                                            tag: "",
                                        }
                                    };
                                    ::slog::Record::new(
                                        &RS,
                                        &::core::fmt::Arguments::new_v1(
                                            &["Serial Task Failed: "],
                                            &match (&e,) {
                                                _args => [::core::fmt::ArgumentV1::new(
                                                    _args.0,
                                                    ::core::fmt::Display::fmt,
                                                )],
                                            },
                                        ),
                                        ::slog::BorrowedKV(&()),
                                    )
                                })
                            };
                            return;
                        }
                    };
                    let config = WebSocketConfig {
                        max_send_queue: Some(4096),
                        ..Default::default()
                    };
                    let ws_stream =
                        WebSocketStream::from_raw_socket(upgraded, Role::Server, Some(config))
                            .await;
                    let _ =
                        instance_serial_task(detach_recv, serial, ws_stream, ws_log, &actx).await;
                });
                context.serial_task = Some(SerialTask { task, detach_ch });
                Ok(Response::builder()
                    .status(StatusCode::SWITCHING_PROTOCOLS)
                    .header(header::CONNECTION, "Upgrade")
                    .header(header::UPGRADE, "websocket")
                    .header(header::SEC_WEBSOCKET_ACCEPT, accept_key)
                    .body(Body::empty())?)
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstancePathParams>,
                ) {
                    future_endpoint_must_be_send(instance_serial(arg0, arg1));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_serial".to_string(),
                instance_serial,
                dropshot::Method::GET,
                "/instances/{instance_id}/serial",
            )
        }
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstancePathParams>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse(
            <Result<HttpResponseUpdatedNoContent, HttpError> as ResultTrait>::T,
        );
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type::<
            <Result<HttpResponseUpdatedNoContent, HttpError> as ResultTrait>::E,
        >();
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_serial_detach
    struct instance_serial_detach {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_serial_detach
    const instance_serial_detach: instance_serial_detach = instance_serial_detach {};
    impl From<instance_serial_detach>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_serial_detach) -> Self {
            async fn instance_serial_detach(
                rqctx: Arc<RequestContext<Context>>,
                path_params: Path<api::InstancePathParams>,
            ) -> Result<HttpResponseUpdatedNoContent, HttpError> {
                let mut context = rqctx.context().context.lock().await;
                let context = context.as_mut().ok_or_else(|| {
                    HttpError::for_internal_error(
                        "Server not initialized (no instance)".to_string(),
                    )
                })?;
                if path_params.into_inner().instance_id != context.properties.id {
                    return Err(HttpError::for_internal_error(
                        "UUID mismatch (path did not match struct)".to_string(),
                    ));
                }
                let serial_task = context
                    .serial_task
                    .take()
                    .filter(|s| s.is_attached())
                    .ok_or_else(|| {
                        HttpError::for_bad_request(
                            None,
                            "serial console already detached".to_string(),
                        )
                    })?;
                serial_task.detach_ch.send(()).map_err(|_| {
                    HttpError::for_internal_error(
                        "couldn't send detach message to serial task".to_string(),
                    )
                })?;
                let _ = serial_task.task.await.map_err(|_| {
                    HttpError::for_internal_error(
                        "failed to complete existing serial task".to_string(),
                    )
                })?;
                Ok(HttpResponseUpdatedNoContent {})
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstancePathParams>,
                ) {
                    future_endpoint_must_be_send(instance_serial_detach(arg0, arg1));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_serial_detach".to_string(),
                instance_serial_detach,
                dropshot::Method::PUT,
                "/instances/{instance_id}/serial/detach",
            )
        }
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstancePathParams>>();
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<TypedBody<api::InstanceMigrateStartRequest>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse(<Result<Response<Body>, HttpError> as ResultTrait>::T);
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type::<<Result<Response<Body>, HttpError> as ResultTrait>::E>();
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_migrate_start
    struct instance_migrate_start {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_migrate_start
    const instance_migrate_start: instance_migrate_start = instance_migrate_start {};
    impl From<instance_migrate_start>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_migrate_start) -> Self {
            async fn instance_migrate_start(
                rqctx: Arc<RequestContext<Context>>,
                path_params: Path<api::InstancePathParams>,
                request: TypedBody<api::InstanceMigrateStartRequest>,
            ) -> Result<Response<Body>, HttpError> {
                let instance_id = path_params.into_inner().instance_id;
                let migration_id = request.into_inner().migration_id;
                migrate::source_start(rqctx, instance_id, migration_id)
                    .await
                    .map_err(Into::into)
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstancePathParams>,
                    arg2: TypedBody<api::InstanceMigrateStartRequest>,
                ) {
                    future_endpoint_must_be_send(instance_migrate_start(arg0, arg1, arg2));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_migrate_start".to_string(),
                instance_migrate_start,
                dropshot::Method::PUT,
                "/instances/{instance_id}/migrate/start",
            )
            .visible(false)
        }
    }
    const _: fn() = || {
        struct NeedRequestContext(
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        );
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<Path<api::InstancePathParams>>();
    };
    const _: fn() = || {
        fn need_extractor<T>()
        where
            T: ?Sized + dropshot::Extractor,
        {
        }
        need_extractor::<TypedBody<api::InstanceMigrateStatusRequest>>();
    };
    const _: fn() = || {
        trait ResultTrait {
            type T;
            type E;
        }
        impl<TT, EE> ResultTrait for Result<TT, EE>
        where
            TT: dropshot::HttpResponse,
        {
            type T = TT;
            type E = EE;
        }
        struct NeedHttpResponse (< Result < HttpResponseOk < api :: InstanceMigrateStatusResponse > , HttpError > as ResultTrait > :: T) ;
        trait TypeEq {
            type This: ?Sized;
        }
        impl<T: ?Sized> TypeEq for T {
            type This = Self;
        }
        fn validate_result_error_type<T>()
        where
            T: ?Sized + TypeEq<This = dropshot::HttpError>,
        {
        }
        validate_result_error_type :: < < Result < HttpResponseOk < api :: InstanceMigrateStatusResponse > , HttpError > as ResultTrait > :: E > () ;
    };
    #[allow(non_camel_case_types, missing_docs)]
    ///API Endpoint: instance_migrate_status
    struct instance_migrate_status {}
    #[allow(non_upper_case_globals, missing_docs)]
    ///API Endpoint: instance_migrate_status
    const instance_migrate_status: instance_migrate_status = instance_migrate_status {};
    impl From<instance_migrate_status>
        for dropshot::ApiEndpoint<
            <Arc<RequestContext<Context>> as dropshot::RequestContextArgument>::Context,
        >
    {
        fn from(_: instance_migrate_status) -> Self {
            async fn instance_migrate_status(
                rqctx: Arc<RequestContext<Context>>,
                _path_params: Path<api::InstancePathParams>,
                request: TypedBody<api::InstanceMigrateStatusRequest>,
            ) -> Result<HttpResponseOk<api::InstanceMigrateStatusResponse>, HttpError> {
                let migration_id = request.into_inner().migration_id;
                migrate::migrate_status(rqctx, migration_id)
                    .await
                    .map_err(Into::into)
                    .map(HttpResponseOk)
            }
            const _: fn() = || {
                fn future_endpoint_must_be_send<T: ::std::marker::Send>(_t: T) {}
                fn check_future_bounds(
                    arg0: Arc<RequestContext<Context>>,
                    arg1: Path<api::InstancePathParams>,
                    arg2: TypedBody<api::InstanceMigrateStatusRequest>,
                ) {
                    future_endpoint_must_be_send(instance_migrate_status(arg0, arg1, arg2));
                }
            };
            dropshot::ApiEndpoint::new(
                "instance_migrate_status".to_string(),
                instance_migrate_status,
                dropshot::Method::GET,
                "/instances/{instance_id}/migrate/status",
            )
        }
    }
    /// Returns a Dropshot [`ApiDescription`] object to launch a server.
    pub fn api() -> ApiDescription<Context> {
        let mut api = ApiDescription::new();
        api.register(instance_ensure).unwrap();
        api.register(instance_get_uuid).unwrap();
        api.register(instance_get).unwrap();
        api.register(instance_state_monitor).unwrap();
        api.register(instance_state_put).unwrap();
        api.register(instance_serial).unwrap();
        api.register(instance_serial_detach).unwrap();
        api.register(instance_migrate_start).unwrap();
        api.register(instance_migrate_status).unwrap();
        api
    }
}
