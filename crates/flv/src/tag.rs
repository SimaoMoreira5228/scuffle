//! FLV Tag processing

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use nutype_enum::nutype_enum;
use scuffle_bytes_util::BytesCursorExt;

use super::audio::AudioData;
use super::script::ScriptData;
use super::video::VideoData;
use crate::error::FlvError;

/// An FLV Tag
///
/// Tags have different types and thus different data structures. To accommodate
/// this the [`FlvTagData`] enum is used.
///
/// Defined by:
/// - Legacy FLV spec, Annex E.4.1
///
/// The v10.1 spec adds some additional fields to the tag to accomodate
/// encryption. We dont support this because it is not needed for our use case.
/// (and I suspect it is not used anywhere anymore.)
#[derive(Debug, Clone, PartialEq)]
pub struct FlvTag<'a> {
    /// The timestamp of this tag in milliseconds
    pub timestamp_ms: u32,
    /// The stream id of this tag
    pub stream_id: u32,
    /// The actual data of the tag
    pub data: FlvTagData<'a>,
}

impl FlvTag<'_> {
    /// Demux a FLV tag from the given reader.
    ///
    /// The reader will be advanced to the end of the tag.
    ///
    /// The reader needs to be a [`std::io::Cursor`] with a [`Bytes`] buffer because we
    /// take advantage of zero-copy reading.
    pub fn demux(reader: &mut std::io::Cursor<Bytes>) -> Result<Self, FlvError> {
        let first_byte = reader.read_u8()?;

        // encrypted
        let filter = (first_byte & 0b0010_0000) != 0;

        // Only the last 5 bits are the tag type.
        let tag_type = FlvTagType::from(first_byte & 0b00011111);

        let data_size = reader.read_u24::<BigEndian>()?;
        // The timestamp bit is weird. Its 24bits but then there is an extended 8 bit
        // number to create a 32bit number.
        let timestamp_ms = reader.read_u24::<BigEndian>()? | ((reader.read_u8()? as u32) << 24);

        // The stream id according to the spec is ALWAYS 0. (likely not true)
        let stream_id = reader.read_u24::<BigEndian>()?;

        // We then extract the data from the reader. (advancing the cursor to the end of
        // the tag)
        let data = reader.extract_bytes(data_size as usize)?;

        let data = if !filter {
            // Finally we demux the data.
            FlvTagData::demux(tag_type, &mut std::io::Cursor::new(data))?
        } else {
            // If the tag is encrypted we just return the data as is.
            FlvTagData::Encrypted { data }
        };

        Ok(FlvTag {
            timestamp_ms,
            stream_id,
            data,
        })
    }
}

nutype_enum! {
    /// FLV Tag Type
    ///
    /// This is the type of the tag.
    ///
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - FLV tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.1 - FLV Tag)
    ///
    /// The 3 types that are supported are:
    /// - Audio(8)
    /// - Video(9)
    /// - ScriptData(18)
    pub enum FlvTagType(u8) {
        /// [`AudioData`]
        Audio = 8,
        /// [`VideoData`]
        Video = 9,
        /// [`ScriptData`]
        ScriptData = 18,
    }
}

/// FLV Tag Data
///
/// This is a container for the actual media data.
/// This enum contains the data for the different types of tags.
///
/// Defined by:
/// - Legacy FLV spec, Annex E.4.1
#[derive(Debug, Clone, PartialEq)]
pub enum FlvTagData<'a> {
    /// AudioData when the FlvTagType is Audio(8)
    ///
    /// Defined by:
    /// - Legacy FLV spec, Annex E.4.2.1
    Audio(AudioData),
    /// VideoData when the FlvTagType is Video(9)
    ///
    /// Defined by:
    /// - Legacy FLV spec, Annex E.4.3.1
    Video(VideoData<'a>),
    /// ScriptData when the FlvTagType is ScriptData(18)
    ///
    /// Defined by:
    /// - Legacy FLV spec, Annex E.4.4.1
    ScriptData(ScriptData<'a>),
    /// Encrypted tag.
    ///
    /// This library neither supports demuxing nor decrypting encrypted tags.
    Encrypted {
        /// The raw unencrypted tag data.
        ///
        /// This includes all data that follows the StreamID field.
        /// See the legacy FLV spec, Annex E.4.1 for more information.
        data: Bytes,
    },
    /// Any tag type that we dont know how to demux, with the corresponding data
    /// being the raw bytes of the tag.
    Unknown {
        /// The tag type.
        tag_type: FlvTagType,
        /// The raw data of the tag.
        data: Bytes,
    },
}

impl FlvTagData<'_> {
    /// Demux a FLV tag data from the given reader.
    ///
    /// The reader will be enirely consumed.
    ///
    /// The reader needs to be a [`std::io::Cursor`] with a [`Bytes`] buffer because we
    /// take advantage of zero-copy reading.
    pub fn demux(tag_type: FlvTagType, reader: &mut std::io::Cursor<Bytes>) -> Result<Self, FlvError> {
        match tag_type {
            FlvTagType::Audio => Ok(FlvTagData::Audio(AudioData::demux(reader)?)),
            FlvTagType::Video => Ok(FlvTagData::Video(VideoData::demux(reader)?)),
            FlvTagType::ScriptData => Ok(FlvTagData::ScriptData(ScriptData::demux(reader)?)),
            _ => Ok(FlvTagData::Unknown {
                tag_type,
                data: reader.extract_remaining(),
            }),
        }
    }
}
