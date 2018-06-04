// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// This file is generated. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]

use protobuf::Message as Message_imported_for_functions;
use protobuf::ProtobufEnum as ProtobufEnum_imported_for_functions;

#[derive(PartialEq,Clone,Default)]
pub struct Map {
    // message fields
    roads: ::protobuf::RepeatedField<Road>,
    intersections: ::protobuf::RepeatedField<Intersection>,
    buildings: ::protobuf::RepeatedField<Building>,
    parcels: ::protobuf::RepeatedField<Parcel>,
    // special fields
    unknown_fields: ::protobuf::UnknownFields,
    cached_size: ::protobuf::CachedSize,
}

// see codegen.rs for the explanation why impl Sync explicitly
unsafe impl ::std::marker::Sync for Map {}

impl Map {
    pub fn new() -> Map {
        ::std::default::Default::default()
    }

    pub fn default_instance() -> &'static Map {
        static mut instance: ::protobuf::lazy::Lazy<Map> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const Map,
        };
        unsafe {
            instance.get(Map::new)
        }
    }

    // repeated .abstreet.Road roads = 1;

    pub fn clear_roads(&mut self) {
        self.roads.clear();
    }

    // Param is passed by value, moved
    pub fn set_roads(&mut self, v: ::protobuf::RepeatedField<Road>) {
        self.roads = v;
    }

    // Mutable pointer to the field.
    pub fn mut_roads(&mut self) -> &mut ::protobuf::RepeatedField<Road> {
        &mut self.roads
    }

    // Take field
    pub fn take_roads(&mut self) -> ::protobuf::RepeatedField<Road> {
        ::std::mem::replace(&mut self.roads, ::protobuf::RepeatedField::new())
    }

    pub fn get_roads(&self) -> &[Road] {
        &self.roads
    }

    fn get_roads_for_reflect(&self) -> &::protobuf::RepeatedField<Road> {
        &self.roads
    }

    fn mut_roads_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<Road> {
        &mut self.roads
    }

    // repeated .abstreet.Intersection intersections = 2;

    pub fn clear_intersections(&mut self) {
        self.intersections.clear();
    }

    // Param is passed by value, moved
    pub fn set_intersections(&mut self, v: ::protobuf::RepeatedField<Intersection>) {
        self.intersections = v;
    }

    // Mutable pointer to the field.
    pub fn mut_intersections(&mut self) -> &mut ::protobuf::RepeatedField<Intersection> {
        &mut self.intersections
    }

    // Take field
    pub fn take_intersections(&mut self) -> ::protobuf::RepeatedField<Intersection> {
        ::std::mem::replace(&mut self.intersections, ::protobuf::RepeatedField::new())
    }

    pub fn get_intersections(&self) -> &[Intersection] {
        &self.intersections
    }

    fn get_intersections_for_reflect(&self) -> &::protobuf::RepeatedField<Intersection> {
        &self.intersections
    }

    fn mut_intersections_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<Intersection> {
        &mut self.intersections
    }

    // repeated .abstreet.Building buildings = 3;

    pub fn clear_buildings(&mut self) {
        self.buildings.clear();
    }

    // Param is passed by value, moved
    pub fn set_buildings(&mut self, v: ::protobuf::RepeatedField<Building>) {
        self.buildings = v;
    }

    // Mutable pointer to the field.
    pub fn mut_buildings(&mut self) -> &mut ::protobuf::RepeatedField<Building> {
        &mut self.buildings
    }

    // Take field
    pub fn take_buildings(&mut self) -> ::protobuf::RepeatedField<Building> {
        ::std::mem::replace(&mut self.buildings, ::protobuf::RepeatedField::new())
    }

    pub fn get_buildings(&self) -> &[Building] {
        &self.buildings
    }

    fn get_buildings_for_reflect(&self) -> &::protobuf::RepeatedField<Building> {
        &self.buildings
    }

    fn mut_buildings_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<Building> {
        &mut self.buildings
    }

    // repeated .abstreet.Parcel parcels = 4;

    pub fn clear_parcels(&mut self) {
        self.parcels.clear();
    }

    // Param is passed by value, moved
    pub fn set_parcels(&mut self, v: ::protobuf::RepeatedField<Parcel>) {
        self.parcels = v;
    }

    // Mutable pointer to the field.
    pub fn mut_parcels(&mut self) -> &mut ::protobuf::RepeatedField<Parcel> {
        &mut self.parcels
    }

    // Take field
    pub fn take_parcels(&mut self) -> ::protobuf::RepeatedField<Parcel> {
        ::std::mem::replace(&mut self.parcels, ::protobuf::RepeatedField::new())
    }

    pub fn get_parcels(&self) -> &[Parcel] {
        &self.parcels
    }

    fn get_parcels_for_reflect(&self) -> &::protobuf::RepeatedField<Parcel> {
        &self.parcels
    }

    fn mut_parcels_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<Parcel> {
        &mut self.parcels
    }
}

impl ::protobuf::Message for Map {
    fn is_initialized(&self) -> bool {
        for v in &self.roads {
            if !v.is_initialized() {
                return false;
            }
        };
        for v in &self.intersections {
            if !v.is_initialized() {
                return false;
            }
        };
        for v in &self.buildings {
            if !v.is_initialized() {
                return false;
            }
        };
        for v in &self.parcels {
            if !v.is_initialized() {
                return false;
            }
        };
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_repeated_message_into(wire_type, is, &mut self.roads)?;
                },
                2 => {
                    ::protobuf::rt::read_repeated_message_into(wire_type, is, &mut self.intersections)?;
                },
                3 => {
                    ::protobuf::rt::read_repeated_message_into(wire_type, is, &mut self.buildings)?;
                },
                4 => {
                    ::protobuf::rt::read_repeated_message_into(wire_type, is, &mut self.parcels)?;
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        for value in &self.roads {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        };
        for value in &self.intersections {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        };
        for value in &self.buildings {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        };
        for value in &self.parcels {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        };
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream) -> ::protobuf::ProtobufResult<()> {
        for v in &self.roads {
            os.write_tag(1, ::protobuf::wire_format::WireTypeLengthDelimited)?;
            os.write_raw_varint32(v.get_cached_size())?;
            v.write_to_with_cached_sizes(os)?;
        };
        for v in &self.intersections {
            os.write_tag(2, ::protobuf::wire_format::WireTypeLengthDelimited)?;
            os.write_raw_varint32(v.get_cached_size())?;
            v.write_to_with_cached_sizes(os)?;
        };
        for v in &self.buildings {
            os.write_tag(3, ::protobuf::wire_format::WireTypeLengthDelimited)?;
            os.write_raw_varint32(v.get_cached_size())?;
            v.write_to_with_cached_sizes(os)?;
        };
        for v in &self.parcels {
            os.write_tag(4, ::protobuf::wire_format::WireTypeLengthDelimited)?;
            os.write_raw_varint32(v.get_cached_size())?;
            v.write_to_with_cached_sizes(os)?;
        };
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &::std::any::Any {
        self as &::std::any::Any
    }
    fn as_any_mut(&mut self) -> &mut ::std::any::Any {
        self as &mut ::std::any::Any
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<::std::any::Any> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        ::protobuf::MessageStatic::descriptor_static(None::<Self>)
    }
}

impl ::protobuf::MessageStatic for Map {
    fn new() -> Map {
        Map::new()
    }

    fn descriptor_static(_: ::std::option::Option<Map>) -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeMessage<Road>>(
                    "roads",
                    Map::get_roads_for_reflect,
                    Map::mut_roads_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeMessage<Intersection>>(
                    "intersections",
                    Map::get_intersections_for_reflect,
                    Map::mut_intersections_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeMessage<Building>>(
                    "buildings",
                    Map::get_buildings_for_reflect,
                    Map::mut_buildings_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeMessage<Parcel>>(
                    "parcels",
                    Map::get_parcels_for_reflect,
                    Map::mut_parcels_for_reflect,
                ));
                ::protobuf::reflect::MessageDescriptor::new::<Map>(
                    "Map",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }
}

impl ::protobuf::Clear for Map {
    fn clear(&mut self) {
        self.clear_roads();
        self.clear_intersections();
        self.clear_buildings();
        self.clear_parcels();
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for Map {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Map {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default)]
pub struct Coordinate {
    // message fields
    latitude: ::std::option::Option<f64>,
    longitude: ::std::option::Option<f64>,
    // special fields
    unknown_fields: ::protobuf::UnknownFields,
    cached_size: ::protobuf::CachedSize,
}

// see codegen.rs for the explanation why impl Sync explicitly
unsafe impl ::std::marker::Sync for Coordinate {}

impl Coordinate {
    pub fn new() -> Coordinate {
        ::std::default::Default::default()
    }

    pub fn default_instance() -> &'static Coordinate {
        static mut instance: ::protobuf::lazy::Lazy<Coordinate> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const Coordinate,
        };
        unsafe {
            instance.get(Coordinate::new)
        }
    }

    // required double latitude = 1;

    pub fn clear_latitude(&mut self) {
        self.latitude = ::std::option::Option::None;
    }

    pub fn has_latitude(&self) -> bool {
        self.latitude.is_some()
    }

    // Param is passed by value, moved
    pub fn set_latitude(&mut self, v: f64) {
        self.latitude = ::std::option::Option::Some(v);
    }

    pub fn get_latitude(&self) -> f64 {
        self.latitude.unwrap_or(0.)
    }

    fn get_latitude_for_reflect(&self) -> &::std::option::Option<f64> {
        &self.latitude
    }

    fn mut_latitude_for_reflect(&mut self) -> &mut ::std::option::Option<f64> {
        &mut self.latitude
    }

    // required double longitude = 2;

    pub fn clear_longitude(&mut self) {
        self.longitude = ::std::option::Option::None;
    }

    pub fn has_longitude(&self) -> bool {
        self.longitude.is_some()
    }

    // Param is passed by value, moved
    pub fn set_longitude(&mut self, v: f64) {
        self.longitude = ::std::option::Option::Some(v);
    }

    pub fn get_longitude(&self) -> f64 {
        self.longitude.unwrap_or(0.)
    }

    fn get_longitude_for_reflect(&self) -> &::std::option::Option<f64> {
        &self.longitude
    }

    fn mut_longitude_for_reflect(&mut self) -> &mut ::std::option::Option<f64> {
        &mut self.longitude
    }
}

impl ::protobuf::Message for Coordinate {
    fn is_initialized(&self) -> bool {
        if self.latitude.is_none() {
            return false;
        }
        if self.longitude.is_none() {
            return false;
        }
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    if wire_type != ::protobuf::wire_format::WireTypeFixed64 {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_double()?;
                    self.latitude = ::std::option::Option::Some(tmp);
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeFixed64 {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_double()?;
                    self.longitude = ::std::option::Option::Some(tmp);
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if let Some(v) = self.latitude {
            my_size += 9;
        }
        if let Some(v) = self.longitude {
            my_size += 9;
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream) -> ::protobuf::ProtobufResult<()> {
        if let Some(v) = self.latitude {
            os.write_double(1, v)?;
        }
        if let Some(v) = self.longitude {
            os.write_double(2, v)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &::std::any::Any {
        self as &::std::any::Any
    }
    fn as_any_mut(&mut self) -> &mut ::std::any::Any {
        self as &mut ::std::any::Any
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<::std::any::Any> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        ::protobuf::MessageStatic::descriptor_static(None::<Self>)
    }
}

impl ::protobuf::MessageStatic for Coordinate {
    fn new() -> Coordinate {
        Coordinate::new()
    }

    fn descriptor_static(_: ::std::option::Option<Coordinate>) -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeDouble>(
                    "latitude",
                    Coordinate::get_latitude_for_reflect,
                    Coordinate::mut_latitude_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeDouble>(
                    "longitude",
                    Coordinate::get_longitude_for_reflect,
                    Coordinate::mut_longitude_for_reflect,
                ));
                ::protobuf::reflect::MessageDescriptor::new::<Coordinate>(
                    "Coordinate",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }
}

impl ::protobuf::Clear for Coordinate {
    fn clear(&mut self) {
        self.clear_latitude();
        self.clear_longitude();
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for Coordinate {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Coordinate {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default)]
pub struct Road {
    // message fields
    points: ::protobuf::RepeatedField<Coordinate>,
    osm_tags: ::protobuf::RepeatedField<::std::string::String>,
    osm_way_id: ::std::option::Option<i64>,
    // special fields
    unknown_fields: ::protobuf::UnknownFields,
    cached_size: ::protobuf::CachedSize,
}

// see codegen.rs for the explanation why impl Sync explicitly
unsafe impl ::std::marker::Sync for Road {}

impl Road {
    pub fn new() -> Road {
        ::std::default::Default::default()
    }

    pub fn default_instance() -> &'static Road {
        static mut instance: ::protobuf::lazy::Lazy<Road> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const Road,
        };
        unsafe {
            instance.get(Road::new)
        }
    }

    // repeated .abstreet.Coordinate points = 1;

    pub fn clear_points(&mut self) {
        self.points.clear();
    }

    // Param is passed by value, moved
    pub fn set_points(&mut self, v: ::protobuf::RepeatedField<Coordinate>) {
        self.points = v;
    }

    // Mutable pointer to the field.
    pub fn mut_points(&mut self) -> &mut ::protobuf::RepeatedField<Coordinate> {
        &mut self.points
    }

    // Take field
    pub fn take_points(&mut self) -> ::protobuf::RepeatedField<Coordinate> {
        ::std::mem::replace(&mut self.points, ::protobuf::RepeatedField::new())
    }

    pub fn get_points(&self) -> &[Coordinate] {
        &self.points
    }

    fn get_points_for_reflect(&self) -> &::protobuf::RepeatedField<Coordinate> {
        &self.points
    }

    fn mut_points_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<Coordinate> {
        &mut self.points
    }

    // repeated string osm_tags = 2;

    pub fn clear_osm_tags(&mut self) {
        self.osm_tags.clear();
    }

    // Param is passed by value, moved
    pub fn set_osm_tags(&mut self, v: ::protobuf::RepeatedField<::std::string::String>) {
        self.osm_tags = v;
    }

    // Mutable pointer to the field.
    pub fn mut_osm_tags(&mut self) -> &mut ::protobuf::RepeatedField<::std::string::String> {
        &mut self.osm_tags
    }

    // Take field
    pub fn take_osm_tags(&mut self) -> ::protobuf::RepeatedField<::std::string::String> {
        ::std::mem::replace(&mut self.osm_tags, ::protobuf::RepeatedField::new())
    }

    pub fn get_osm_tags(&self) -> &[::std::string::String] {
        &self.osm_tags
    }

    fn get_osm_tags_for_reflect(&self) -> &::protobuf::RepeatedField<::std::string::String> {
        &self.osm_tags
    }

    fn mut_osm_tags_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<::std::string::String> {
        &mut self.osm_tags
    }

    // required int64 osm_way_id = 3;

    pub fn clear_osm_way_id(&mut self) {
        self.osm_way_id = ::std::option::Option::None;
    }

    pub fn has_osm_way_id(&self) -> bool {
        self.osm_way_id.is_some()
    }

    // Param is passed by value, moved
    pub fn set_osm_way_id(&mut self, v: i64) {
        self.osm_way_id = ::std::option::Option::Some(v);
    }

    pub fn get_osm_way_id(&self) -> i64 {
        self.osm_way_id.unwrap_or(0)
    }

    fn get_osm_way_id_for_reflect(&self) -> &::std::option::Option<i64> {
        &self.osm_way_id
    }

    fn mut_osm_way_id_for_reflect(&mut self) -> &mut ::std::option::Option<i64> {
        &mut self.osm_way_id
    }
}

impl ::protobuf::Message for Road {
    fn is_initialized(&self) -> bool {
        if self.osm_way_id.is_none() {
            return false;
        }
        for v in &self.points {
            if !v.is_initialized() {
                return false;
            }
        };
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_repeated_message_into(wire_type, is, &mut self.points)?;
                },
                2 => {
                    ::protobuf::rt::read_repeated_string_into(wire_type, is, &mut self.osm_tags)?;
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_int64()?;
                    self.osm_way_id = ::std::option::Option::Some(tmp);
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        for value in &self.points {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        };
        for value in &self.osm_tags {
            my_size += ::protobuf::rt::string_size(2, &value);
        };
        if let Some(v) = self.osm_way_id {
            my_size += ::protobuf::rt::value_size(3, v, ::protobuf::wire_format::WireTypeVarint);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream) -> ::protobuf::ProtobufResult<()> {
        for v in &self.points {
            os.write_tag(1, ::protobuf::wire_format::WireTypeLengthDelimited)?;
            os.write_raw_varint32(v.get_cached_size())?;
            v.write_to_with_cached_sizes(os)?;
        };
        for v in &self.osm_tags {
            os.write_string(2, &v)?;
        };
        if let Some(v) = self.osm_way_id {
            os.write_int64(3, v)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &::std::any::Any {
        self as &::std::any::Any
    }
    fn as_any_mut(&mut self) -> &mut ::std::any::Any {
        self as &mut ::std::any::Any
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<::std::any::Any> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        ::protobuf::MessageStatic::descriptor_static(None::<Self>)
    }
}

impl ::protobuf::MessageStatic for Road {
    fn new() -> Road {
        Road::new()
    }

    fn descriptor_static(_: ::std::option::Option<Road>) -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeMessage<Coordinate>>(
                    "points",
                    Road::get_points_for_reflect,
                    Road::mut_points_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeString>(
                    "osm_tags",
                    Road::get_osm_tags_for_reflect,
                    Road::mut_osm_tags_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeInt64>(
                    "osm_way_id",
                    Road::get_osm_way_id_for_reflect,
                    Road::mut_osm_way_id_for_reflect,
                ));
                ::protobuf::reflect::MessageDescriptor::new::<Road>(
                    "Road",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }
}

impl ::protobuf::Clear for Road {
    fn clear(&mut self) {
        self.clear_points();
        self.clear_osm_tags();
        self.clear_osm_way_id();
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for Road {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Road {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default)]
pub struct Intersection {
    // message fields
    point: ::protobuf::SingularPtrField<Coordinate>,
    elevation_meters: ::std::option::Option<f64>,
    has_traffic_signal: ::std::option::Option<bool>,
    // special fields
    unknown_fields: ::protobuf::UnknownFields,
    cached_size: ::protobuf::CachedSize,
}

// see codegen.rs for the explanation why impl Sync explicitly
unsafe impl ::std::marker::Sync for Intersection {}

impl Intersection {
    pub fn new() -> Intersection {
        ::std::default::Default::default()
    }

    pub fn default_instance() -> &'static Intersection {
        static mut instance: ::protobuf::lazy::Lazy<Intersection> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const Intersection,
        };
        unsafe {
            instance.get(Intersection::new)
        }
    }

    // required .abstreet.Coordinate point = 1;

    pub fn clear_point(&mut self) {
        self.point.clear();
    }

    pub fn has_point(&self) -> bool {
        self.point.is_some()
    }

    // Param is passed by value, moved
    pub fn set_point(&mut self, v: Coordinate) {
        self.point = ::protobuf::SingularPtrField::some(v);
    }

    // Mutable pointer to the field.
    // If field is not initialized, it is initialized with default value first.
    pub fn mut_point(&mut self) -> &mut Coordinate {
        if self.point.is_none() {
            self.point.set_default();
        }
        self.point.as_mut().unwrap()
    }

    // Take field
    pub fn take_point(&mut self) -> Coordinate {
        self.point.take().unwrap_or_else(|| Coordinate::new())
    }

    pub fn get_point(&self) -> &Coordinate {
        self.point.as_ref().unwrap_or_else(|| Coordinate::default_instance())
    }

    fn get_point_for_reflect(&self) -> &::protobuf::SingularPtrField<Coordinate> {
        &self.point
    }

    fn mut_point_for_reflect(&mut self) -> &mut ::protobuf::SingularPtrField<Coordinate> {
        &mut self.point
    }

    // required double elevation_meters = 2;

    pub fn clear_elevation_meters(&mut self) {
        self.elevation_meters = ::std::option::Option::None;
    }

    pub fn has_elevation_meters(&self) -> bool {
        self.elevation_meters.is_some()
    }

    // Param is passed by value, moved
    pub fn set_elevation_meters(&mut self, v: f64) {
        self.elevation_meters = ::std::option::Option::Some(v);
    }

    pub fn get_elevation_meters(&self) -> f64 {
        self.elevation_meters.unwrap_or(0.)
    }

    fn get_elevation_meters_for_reflect(&self) -> &::std::option::Option<f64> {
        &self.elevation_meters
    }

    fn mut_elevation_meters_for_reflect(&mut self) -> &mut ::std::option::Option<f64> {
        &mut self.elevation_meters
    }

    // required bool has_traffic_signal = 3;

    pub fn clear_has_traffic_signal(&mut self) {
        self.has_traffic_signal = ::std::option::Option::None;
    }

    pub fn has_has_traffic_signal(&self) -> bool {
        self.has_traffic_signal.is_some()
    }

    // Param is passed by value, moved
    pub fn set_has_traffic_signal(&mut self, v: bool) {
        self.has_traffic_signal = ::std::option::Option::Some(v);
    }

    pub fn get_has_traffic_signal(&self) -> bool {
        self.has_traffic_signal.unwrap_or(false)
    }

    fn get_has_traffic_signal_for_reflect(&self) -> &::std::option::Option<bool> {
        &self.has_traffic_signal
    }

    fn mut_has_traffic_signal_for_reflect(&mut self) -> &mut ::std::option::Option<bool> {
        &mut self.has_traffic_signal
    }
}

impl ::protobuf::Message for Intersection {
    fn is_initialized(&self) -> bool {
        if self.point.is_none() {
            return false;
        }
        if self.elevation_meters.is_none() {
            return false;
        }
        if self.has_traffic_signal.is_none() {
            return false;
        }
        for v in &self.point {
            if !v.is_initialized() {
                return false;
            }
        };
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_singular_message_into(wire_type, is, &mut self.point)?;
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeFixed64 {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_double()?;
                    self.elevation_meters = ::std::option::Option::Some(tmp);
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_bool()?;
                    self.has_traffic_signal = ::std::option::Option::Some(tmp);
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if let Some(ref v) = self.point.as_ref() {
            let len = v.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        }
        if let Some(v) = self.elevation_meters {
            my_size += 9;
        }
        if let Some(v) = self.has_traffic_signal {
            my_size += 2;
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream) -> ::protobuf::ProtobufResult<()> {
        if let Some(ref v) = self.point.as_ref() {
            os.write_tag(1, ::protobuf::wire_format::WireTypeLengthDelimited)?;
            os.write_raw_varint32(v.get_cached_size())?;
            v.write_to_with_cached_sizes(os)?;
        }
        if let Some(v) = self.elevation_meters {
            os.write_double(2, v)?;
        }
        if let Some(v) = self.has_traffic_signal {
            os.write_bool(3, v)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &::std::any::Any {
        self as &::std::any::Any
    }
    fn as_any_mut(&mut self) -> &mut ::std::any::Any {
        self as &mut ::std::any::Any
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<::std::any::Any> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        ::protobuf::MessageStatic::descriptor_static(None::<Self>)
    }
}

impl ::protobuf::MessageStatic for Intersection {
    fn new() -> Intersection {
        Intersection::new()
    }

    fn descriptor_static(_: ::std::option::Option<Intersection>) -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_singular_ptr_field_accessor::<_, ::protobuf::types::ProtobufTypeMessage<Coordinate>>(
                    "point",
                    Intersection::get_point_for_reflect,
                    Intersection::mut_point_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeDouble>(
                    "elevation_meters",
                    Intersection::get_elevation_meters_for_reflect,
                    Intersection::mut_elevation_meters_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeBool>(
                    "has_traffic_signal",
                    Intersection::get_has_traffic_signal_for_reflect,
                    Intersection::mut_has_traffic_signal_for_reflect,
                ));
                ::protobuf::reflect::MessageDescriptor::new::<Intersection>(
                    "Intersection",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }
}

impl ::protobuf::Clear for Intersection {
    fn clear(&mut self) {
        self.clear_point();
        self.clear_elevation_meters();
        self.clear_has_traffic_signal();
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for Intersection {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Intersection {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default)]
pub struct Building {
    // message fields
    points: ::protobuf::RepeatedField<Coordinate>,
    osm_tags: ::protobuf::RepeatedField<::std::string::String>,
    osm_way_id: ::std::option::Option<i64>,
    // special fields
    unknown_fields: ::protobuf::UnknownFields,
    cached_size: ::protobuf::CachedSize,
}

// see codegen.rs for the explanation why impl Sync explicitly
unsafe impl ::std::marker::Sync for Building {}

impl Building {
    pub fn new() -> Building {
        ::std::default::Default::default()
    }

    pub fn default_instance() -> &'static Building {
        static mut instance: ::protobuf::lazy::Lazy<Building> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const Building,
        };
        unsafe {
            instance.get(Building::new)
        }
    }

    // repeated .abstreet.Coordinate points = 1;

    pub fn clear_points(&mut self) {
        self.points.clear();
    }

    // Param is passed by value, moved
    pub fn set_points(&mut self, v: ::protobuf::RepeatedField<Coordinate>) {
        self.points = v;
    }

    // Mutable pointer to the field.
    pub fn mut_points(&mut self) -> &mut ::protobuf::RepeatedField<Coordinate> {
        &mut self.points
    }

    // Take field
    pub fn take_points(&mut self) -> ::protobuf::RepeatedField<Coordinate> {
        ::std::mem::replace(&mut self.points, ::protobuf::RepeatedField::new())
    }

    pub fn get_points(&self) -> &[Coordinate] {
        &self.points
    }

    fn get_points_for_reflect(&self) -> &::protobuf::RepeatedField<Coordinate> {
        &self.points
    }

    fn mut_points_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<Coordinate> {
        &mut self.points
    }

    // repeated string osm_tags = 2;

    pub fn clear_osm_tags(&mut self) {
        self.osm_tags.clear();
    }

    // Param is passed by value, moved
    pub fn set_osm_tags(&mut self, v: ::protobuf::RepeatedField<::std::string::String>) {
        self.osm_tags = v;
    }

    // Mutable pointer to the field.
    pub fn mut_osm_tags(&mut self) -> &mut ::protobuf::RepeatedField<::std::string::String> {
        &mut self.osm_tags
    }

    // Take field
    pub fn take_osm_tags(&mut self) -> ::protobuf::RepeatedField<::std::string::String> {
        ::std::mem::replace(&mut self.osm_tags, ::protobuf::RepeatedField::new())
    }

    pub fn get_osm_tags(&self) -> &[::std::string::String] {
        &self.osm_tags
    }

    fn get_osm_tags_for_reflect(&self) -> &::protobuf::RepeatedField<::std::string::String> {
        &self.osm_tags
    }

    fn mut_osm_tags_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<::std::string::String> {
        &mut self.osm_tags
    }

    // required int64 osm_way_id = 3;

    pub fn clear_osm_way_id(&mut self) {
        self.osm_way_id = ::std::option::Option::None;
    }

    pub fn has_osm_way_id(&self) -> bool {
        self.osm_way_id.is_some()
    }

    // Param is passed by value, moved
    pub fn set_osm_way_id(&mut self, v: i64) {
        self.osm_way_id = ::std::option::Option::Some(v);
    }

    pub fn get_osm_way_id(&self) -> i64 {
        self.osm_way_id.unwrap_or(0)
    }

    fn get_osm_way_id_for_reflect(&self) -> &::std::option::Option<i64> {
        &self.osm_way_id
    }

    fn mut_osm_way_id_for_reflect(&mut self) -> &mut ::std::option::Option<i64> {
        &mut self.osm_way_id
    }
}

impl ::protobuf::Message for Building {
    fn is_initialized(&self) -> bool {
        if self.osm_way_id.is_none() {
            return false;
        }
        for v in &self.points {
            if !v.is_initialized() {
                return false;
            }
        };
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_repeated_message_into(wire_type, is, &mut self.points)?;
                },
                2 => {
                    ::protobuf::rt::read_repeated_string_into(wire_type, is, &mut self.osm_tags)?;
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_int64()?;
                    self.osm_way_id = ::std::option::Option::Some(tmp);
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        for value in &self.points {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        };
        for value in &self.osm_tags {
            my_size += ::protobuf::rt::string_size(2, &value);
        };
        if let Some(v) = self.osm_way_id {
            my_size += ::protobuf::rt::value_size(3, v, ::protobuf::wire_format::WireTypeVarint);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream) -> ::protobuf::ProtobufResult<()> {
        for v in &self.points {
            os.write_tag(1, ::protobuf::wire_format::WireTypeLengthDelimited)?;
            os.write_raw_varint32(v.get_cached_size())?;
            v.write_to_with_cached_sizes(os)?;
        };
        for v in &self.osm_tags {
            os.write_string(2, &v)?;
        };
        if let Some(v) = self.osm_way_id {
            os.write_int64(3, v)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &::std::any::Any {
        self as &::std::any::Any
    }
    fn as_any_mut(&mut self) -> &mut ::std::any::Any {
        self as &mut ::std::any::Any
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<::std::any::Any> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        ::protobuf::MessageStatic::descriptor_static(None::<Self>)
    }
}

impl ::protobuf::MessageStatic for Building {
    fn new() -> Building {
        Building::new()
    }

    fn descriptor_static(_: ::std::option::Option<Building>) -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeMessage<Coordinate>>(
                    "points",
                    Building::get_points_for_reflect,
                    Building::mut_points_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeString>(
                    "osm_tags",
                    Building::get_osm_tags_for_reflect,
                    Building::mut_osm_tags_for_reflect,
                ));
                fields.push(::protobuf::reflect::accessor::make_option_accessor::<_, ::protobuf::types::ProtobufTypeInt64>(
                    "osm_way_id",
                    Building::get_osm_way_id_for_reflect,
                    Building::mut_osm_way_id_for_reflect,
                ));
                ::protobuf::reflect::MessageDescriptor::new::<Building>(
                    "Building",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }
}

impl ::protobuf::Clear for Building {
    fn clear(&mut self) {
        self.clear_points();
        self.clear_osm_tags();
        self.clear_osm_way_id();
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for Building {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Building {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Message(self)
    }
}

#[derive(PartialEq,Clone,Default)]
pub struct Parcel {
    // message fields
    points: ::protobuf::RepeatedField<Coordinate>,
    // special fields
    unknown_fields: ::protobuf::UnknownFields,
    cached_size: ::protobuf::CachedSize,
}

// see codegen.rs for the explanation why impl Sync explicitly
unsafe impl ::std::marker::Sync for Parcel {}

impl Parcel {
    pub fn new() -> Parcel {
        ::std::default::Default::default()
    }

    pub fn default_instance() -> &'static Parcel {
        static mut instance: ::protobuf::lazy::Lazy<Parcel> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const Parcel,
        };
        unsafe {
            instance.get(Parcel::new)
        }
    }

    // repeated .abstreet.Coordinate points = 1;

    pub fn clear_points(&mut self) {
        self.points.clear();
    }

    // Param is passed by value, moved
    pub fn set_points(&mut self, v: ::protobuf::RepeatedField<Coordinate>) {
        self.points = v;
    }

    // Mutable pointer to the field.
    pub fn mut_points(&mut self) -> &mut ::protobuf::RepeatedField<Coordinate> {
        &mut self.points
    }

    // Take field
    pub fn take_points(&mut self) -> ::protobuf::RepeatedField<Coordinate> {
        ::std::mem::replace(&mut self.points, ::protobuf::RepeatedField::new())
    }

    pub fn get_points(&self) -> &[Coordinate] {
        &self.points
    }

    fn get_points_for_reflect(&self) -> &::protobuf::RepeatedField<Coordinate> {
        &self.points
    }

    fn mut_points_for_reflect(&mut self) -> &mut ::protobuf::RepeatedField<Coordinate> {
        &mut self.points
    }
}

impl ::protobuf::Message for Parcel {
    fn is_initialized(&self) -> bool {
        for v in &self.points {
            if !v.is_initialized() {
                return false;
            }
        };
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_repeated_message_into(wire_type, is, &mut self.points)?;
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        for value in &self.points {
            let len = value.compute_size();
            my_size += 1 + ::protobuf::rt::compute_raw_varint32_size(len) + len;
        };
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream) -> ::protobuf::ProtobufResult<()> {
        for v in &self.points {
            os.write_tag(1, ::protobuf::wire_format::WireTypeLengthDelimited)?;
            os.write_raw_varint32(v.get_cached_size())?;
            v.write_to_with_cached_sizes(os)?;
        };
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &::std::any::Any {
        self as &::std::any::Any
    }
    fn as_any_mut(&mut self) -> &mut ::std::any::Any {
        self as &mut ::std::any::Any
    }
    fn into_any(self: Box<Self>) -> ::std::boxed::Box<::std::any::Any> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        ::protobuf::MessageStatic::descriptor_static(None::<Self>)
    }
}

impl ::protobuf::MessageStatic for Parcel {
    fn new() -> Parcel {
        Parcel::new()
    }

    fn descriptor_static(_: ::std::option::Option<Parcel>) -> &'static ::protobuf::reflect::MessageDescriptor {
        static mut descriptor: ::protobuf::lazy::Lazy<::protobuf::reflect::MessageDescriptor> = ::protobuf::lazy::Lazy {
            lock: ::protobuf::lazy::ONCE_INIT,
            ptr: 0 as *const ::protobuf::reflect::MessageDescriptor,
        };
        unsafe {
            descriptor.get(|| {
                let mut fields = ::std::vec::Vec::new();
                fields.push(::protobuf::reflect::accessor::make_repeated_field_accessor::<_, ::protobuf::types::ProtobufTypeMessage<Coordinate>>(
                    "points",
                    Parcel::get_points_for_reflect,
                    Parcel::mut_points_for_reflect,
                ));
                ::protobuf::reflect::MessageDescriptor::new::<Parcel>(
                    "Parcel",
                    fields,
                    file_descriptor_proto()
                )
            })
        }
    }
}

impl ::protobuf::Clear for Parcel {
    fn clear(&mut self) {
        self.clear_points();
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for Parcel {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Parcel {
    fn as_ref(&self) -> ::protobuf::reflect::ProtobufValueRef {
        ::protobuf::reflect::ProtobufValueRef::Message(self)
    }
}

static file_descriptor_proto_data: &'static [u8] = b"\
    \n\x0eabstreet.proto\x12\x08abstreet\"\x9d\x01\n\x03Map\x12\x1d\n\x05roa\
    ds\x18\x01\x20\x03(\x0b2\x0e.abstreet.Road\x12-\n\rintersections\x18\x02\
    \x20\x03(\x0b2\x16.abstreet.Intersection\x12%\n\tbuildings\x18\x03\x20\
    \x03(\x0b2\x12.abstreet.Building\x12!\n\x07parcels\x18\x04\x20\x03(\x0b2\
    \x10.abstreet.Parcel\"1\n\nCoordinate\x12\x10\n\x08latitude\x18\x01\x20\
    \x02(\x01\x12\x11\n\tlongitude\x18\x02\x20\x02(\x01\"R\n\x04Road\x12$\n\
    \x06points\x18\x01\x20\x03(\x0b2\x14.abstreet.Coordinate\x12\x10\n\x08os\
    m_tags\x18\x02\x20\x03(\t\x12\x12\n\nosm_way_id\x18\x03\x20\x02(\x03\"i\
    \n\x0cIntersection\x12#\n\x05point\x18\x01\x20\x02(\x0b2\x14.abstreet.Co\
    ordinate\x12\x18\n\x10elevation_meters\x18\x02\x20\x02(\x01\x12\x1a\n\
    \x12has_traffic_signal\x18\x03\x20\x02(\x08\"V\n\x08Building\x12$\n\x06p\
    oints\x18\x01\x20\x03(\x0b2\x14.abstreet.Coordinate\x12\x10\n\x08osm_tag\
    s\x18\x02\x20\x03(\t\x12\x12\n\nosm_way_id\x18\x03\x20\x02(\x03\".\n\x06\
    Parcel\x12$\n\x06points\x18\x01\x20\x03(\x0b2\x14.abstreet.CoordinateJ\
    \xce\x0c\n\x06\x12\x04\0\0-\x01\n\x08\n\x01\x02\x12\x03\x02\x08\x10\n\n\
    \n\x02\x04\0\x12\x04\x04\0\t\x01\n\n\n\x03\x04\0\x01\x12\x03\x04\x08\x0b\
    \n\x0b\n\x04\x04\0\x02\0\x12\x03\x05\x08\x20\n\x0c\n\x05\x04\0\x02\0\x04\
    \x12\x03\x05\x08\x10\n\x0c\n\x05\x04\0\x02\0\x06\x12\x03\x05\x11\x15\n\
    \x0c\n\x05\x04\0\x02\0\x01\x12\x03\x05\x16\x1b\n\x0c\n\x05\x04\0\x02\0\
    \x03\x12\x03\x05\x1e\x1f\n\x0b\n\x04\x04\0\x02\x01\x12\x03\x06\x080\n\
    \x0c\n\x05\x04\0\x02\x01\x04\x12\x03\x06\x08\x10\n\x0c\n\x05\x04\0\x02\
    \x01\x06\x12\x03\x06\x11\x1d\n\x0c\n\x05\x04\0\x02\x01\x01\x12\x03\x06\
    \x1e+\n\x0c\n\x05\x04\0\x02\x01\x03\x12\x03\x06./\n\x0b\n\x04\x04\0\x02\
    \x02\x12\x03\x07\x08(\n\x0c\n\x05\x04\0\x02\x02\x04\x12\x03\x07\x08\x10\
    \n\x0c\n\x05\x04\0\x02\x02\x06\x12\x03\x07\x11\x19\n\x0c\n\x05\x04\0\x02\
    \x02\x01\x12\x03\x07\x1a#\n\x0c\n\x05\x04\0\x02\x02\x03\x12\x03\x07&'\n\
    \x0b\n\x04\x04\0\x02\x03\x12\x03\x08\x08$\n\x0c\n\x05\x04\0\x02\x03\x04\
    \x12\x03\x08\x08\x10\n\x0c\n\x05\x04\0\x02\x03\x06\x12\x03\x08\x11\x17\n\
    \x0c\n\x05\x04\0\x02\x03\x01\x12\x03\x08\x18\x1f\n\x0c\n\x05\x04\0\x02\
    \x03\x03\x12\x03\x08\"#\n\n\n\x02\x04\x01\x12\x04\x0b\0\x10\x01\n\n\n\
    \x03\x04\x01\x01\x12\x03\x0b\x08\x12\n\x10\n\x04\x04\x01\x02\0\x12\x03\r\
    \x08%\x1a\x03\x20y\n\n\x0c\n\x05\x04\x01\x02\0\x04\x12\x03\r\x08\x10\n\
    \x0c\n\x05\x04\x01\x02\0\x05\x12\x03\r\x11\x17\n\x0c\n\x05\x04\x01\x02\0\
    \x01\x12\x03\r\x18\x20\n\x0c\n\x05\x04\x01\x02\0\x03\x12\x03\r#$\n\x10\n\
    \x04\x04\x01\x02\x01\x12\x03\x0f\x08&\x1a\x03\x20x\n\n\x0c\n\x05\x04\x01\
    \x02\x01\x04\x12\x03\x0f\x08\x10\n\x0c\n\x05\x04\x01\x02\x01\x05\x12\x03\
    \x0f\x11\x17\n\x0c\n\x05\x04\x01\x02\x01\x01\x12\x03\x0f\x18!\n\x0c\n\
    \x05\x04\x01\x02\x01\x03\x12\x03\x0f$%\n\n\n\x02\x04\x02\x12\x04\x12\0\
    \x17\x01\n\n\n\x03\x04\x02\x01\x12\x03\x12\x08\x0c\n\x0b\n\x04\x04\x02\
    \x02\0\x12\x03\x13\x08'\n\x0c\n\x05\x04\x02\x02\0\x04\x12\x03\x13\x08\
    \x10\n\x0c\n\x05\x04\x02\x02\0\x06\x12\x03\x13\x11\x1b\n\x0c\n\x05\x04\
    \x02\x02\0\x01\x12\x03\x13\x1c\"\n\x0c\n\x05\x04\x02\x02\0\x03\x12\x03\
    \x13%&\n*\n\x04\x04\x02\x02\x01\x12\x03\x15\x08%\x1a\x1d\x20\"key=value\
    \"\x20format,\x20for\x20now\n\n\x0c\n\x05\x04\x02\x02\x01\x04\x12\x03\
    \x15\x08\x10\n\x0c\n\x05\x04\x02\x02\x01\x05\x12\x03\x15\x11\x17\n\x0c\n\
    \x05\x04\x02\x02\x01\x01\x12\x03\x15\x18\x20\n\x0c\n\x05\x04\x02\x02\x01\
    \x03\x12\x03\x15#$\n\x0b\n\x04\x04\x02\x02\x02\x12\x03\x16\x08&\n\x0c\n\
    \x05\x04\x02\x02\x02\x04\x12\x03\x16\x08\x10\n\x0c\n\x05\x04\x02\x02\x02\
    \x05\x12\x03\x16\x11\x16\n\x0c\n\x05\x04\x02\x02\x02\x01\x12\x03\x16\x17\
    !\n\x0c\n\x05\x04\x02\x02\x02\x03\x12\x03\x16$%\n\n\n\x02\x04\x03\x12\
    \x04\x19\0\x1d\x01\n\n\n\x03\x04\x03\x01\x12\x03\x19\x08\x14\n\x0b\n\x04\
    \x04\x03\x02\0\x12\x03\x1a\x08&\n\x0c\n\x05\x04\x03\x02\0\x04\x12\x03\
    \x1a\x08\x10\n\x0c\n\x05\x04\x03\x02\0\x06\x12\x03\x1a\x11\x1b\n\x0c\n\
    \x05\x04\x03\x02\0\x01\x12\x03\x1a\x1c!\n\x0c\n\x05\x04\x03\x02\0\x03\
    \x12\x03\x1a$%\n\x0b\n\x04\x04\x03\x02\x01\x12\x03\x1b\x08-\n\x0c\n\x05\
    \x04\x03\x02\x01\x04\x12\x03\x1b\x08\x10\n\x0c\n\x05\x04\x03\x02\x01\x05\
    \x12\x03\x1b\x11\x17\n\x0c\n\x05\x04\x03\x02\x01\x01\x12\x03\x1b\x18(\n\
    \x0c\n\x05\x04\x03\x02\x01\x03\x12\x03\x1b+,\n\x0b\n\x04\x04\x03\x02\x02\
    \x12\x03\x1c\x08-\n\x0c\n\x05\x04\x03\x02\x02\x04\x12\x03\x1c\x08\x10\n\
    \x0c\n\x05\x04\x03\x02\x02\x05\x12\x03\x1c\x11\x15\n\x0c\n\x05\x04\x03\
    \x02\x02\x01\x12\x03\x1c\x16(\n\x0c\n\x05\x04\x03\x02\x02\x03\x12\x03\
    \x1c+,\nL\n\x02\x04\x04\x12\x04\x20\0&\x01\x1a@\x20TODO\x20identical\x20\
    to\x20Road.\x20worth\x20representing\x20these\x20the\x20same\x20way?\n\n\
    \n\n\x03\x04\x04\x01\x12\x03\x20\x08\x10\n*\n\x04\x04\x04\x02\0\x12\x03\
    \"\x08'\x1a\x1d\x20last\x20point\x20never\x20the\x20first?\n\n\x0c\n\x05\
    \x04\x04\x02\0\x04\x12\x03\"\x08\x10\n\x0c\n\x05\x04\x04\x02\0\x06\x12\
    \x03\"\x11\x1b\n\x0c\n\x05\x04\x04\x02\0\x01\x12\x03\"\x1c\"\n\x0c\n\x05\
    \x04\x04\x02\0\x03\x12\x03\"%&\n*\n\x04\x04\x04\x02\x01\x12\x03$\x08%\
    \x1a\x1d\x20\"key=value\"\x20format,\x20for\x20now\n\n\x0c\n\x05\x04\x04\
    \x02\x01\x04\x12\x03$\x08\x10\n\x0c\n\x05\x04\x04\x02\x01\x05\x12\x03$\
    \x11\x17\n\x0c\n\x05\x04\x04\x02\x01\x01\x12\x03$\x18\x20\n\x0c\n\x05\
    \x04\x04\x02\x01\x03\x12\x03$#$\n\x0b\n\x04\x04\x04\x02\x02\x12\x03%\x08\
    &\n\x0c\n\x05\x04\x04\x02\x02\x04\x12\x03%\x08\x10\n\x0c\n\x05\x04\x04\
    \x02\x02\x05\x12\x03%\x11\x16\n\x0c\n\x05\x04\x04\x02\x02\x01\x12\x03%\
    \x17!\n\x0c\n\x05\x04\x04\x02\x02\x03\x12\x03%$%\n\n\n\x02\x04\x05\x12\
    \x04(\0-\x01\n\n\n\x03\x04\x05\x01\x12\x03(\x08\x0e\n\xbd\x01\n\x04\x04\
    \x05\x02\0\x12\x03*\x08'\x1a\x1d\x20last\x20point\x20never\x20the\x20fir\
    st?\n\"\x90\x01\x20TODO\x20decide\x20what\x20metadata\x20from\x20the\x20\
    shapefile\x20is\x20useful\n\x20TODO\x20associate\x20a\x20list\x20of\x20b\
    uildings\x20with\x20the\x20parcel,\x20assuming\x20no\x20building\x20span\
    s\x20parcels\n\n\x0c\n\x05\x04\x05\x02\0\x04\x12\x03*\x08\x10\n\x0c\n\
    \x05\x04\x05\x02\0\x06\x12\x03*\x11\x1b\n\x0c\n\x05\x04\x05\x02\0\x01\
    \x12\x03*\x1c\"\n\x0c\n\x05\x04\x05\x02\0\x03\x12\x03*%&\
";

static mut file_descriptor_proto_lazy: ::protobuf::lazy::Lazy<::protobuf::descriptor::FileDescriptorProto> = ::protobuf::lazy::Lazy {
    lock: ::protobuf::lazy::ONCE_INIT,
    ptr: 0 as *const ::protobuf::descriptor::FileDescriptorProto,
};

fn parse_descriptor_proto() -> ::protobuf::descriptor::FileDescriptorProto {
    ::protobuf::parse_from_bytes(file_descriptor_proto_data).unwrap()
}

pub fn file_descriptor_proto() -> &'static ::protobuf::descriptor::FileDescriptorProto {
    unsafe {
        file_descriptor_proto_lazy.get(|| {
            parse_descriptor_proto()
        })
    }
}
