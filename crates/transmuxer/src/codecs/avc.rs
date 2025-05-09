use bytes::Bytes;
use scuffle_flv::video::header::VideoFrameType;
use scuffle_h264::{AVCDecoderConfigurationRecord, Sps};
use scuffle_mp4::DynBox;
use scuffle_mp4::types::avc1::Avc1;
use scuffle_mp4::types::avcc::AvcC;
use scuffle_mp4::types::colr::{ColorType, Colr};
use scuffle_mp4::types::stsd::{SampleEntry, VisualSampleEntry};
use scuffle_mp4::types::trun::{TrunSample, TrunSampleFlag};

use crate::TransmuxError;

pub(crate) fn stsd_entry(config: AVCDecoderConfigurationRecord, sps: &Sps) -> Result<DynBox, TransmuxError> {
    if config.sps.is_empty() {
        return Err(TransmuxError::InvalidAVCDecoderConfigurationRecord);
    }

    let colr = sps.color_config.as_ref().map(|color_config| {
        Colr::new(ColorType::Nclx {
            color_primaries: color_config.color_primaries as u16,
            matrix_coefficients: color_config.matrix_coefficients as u16,
            transfer_characteristics: color_config.transfer_characteristics as u16,
            full_range_flag: color_config.video_full_range_flag,
        })
    });

    Ok(Avc1::new(
        SampleEntry::new(VisualSampleEntry::new(sps.width() as u16, sps.height() as u16, colr)),
        AvcC::new(config),
        None,
    )
    .into())
}

pub(crate) fn trun_sample(
    frame_type: VideoFrameType,
    composition_time: u32,
    duration: u32,
    data: &Bytes,
) -> Result<TrunSample, TransmuxError> {
    Ok(TrunSample {
        composition_time_offset: Some(composition_time as i64),
        duration: Some(duration),
        flags: Some(TrunSampleFlag {
            reserved: 0,
            is_leading: 0,
            sample_degradation_priority: 0,
            sample_depends_on: if frame_type == VideoFrameType::KeyFrame { 2 } else { 1 },
            sample_has_redundancy: 0,
            sample_is_depended_on: 0,
            sample_is_non_sync_sample: frame_type != VideoFrameType::KeyFrame,
            sample_padding_value: 0,
        }),
        size: Some(data.len() as u32),
    })
}
