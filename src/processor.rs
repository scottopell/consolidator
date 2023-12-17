use core::result::Result;
use std::{fs::File, path::Path};

use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as SymphoniaError,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};
use thiserror::Error as ThisError;
use tracing::{debug, error, info, warn};

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
}

fn get_sample_buf(file: File) -> Result<symphonia::core::audio::SampleBuffer<f32>, SymphoniaError> {
    let file = Box::new(file);
    // Create the media source stream using the boxed media source from above.
    let mss = MediaSourceStream::new(file, MediaSourceStreamOptions::default());

    // Create a hint to help the format registry guess what format reader is appropriate. In this
    // example we'll leave it empty.
    let hint = Hint::new();

    // Use the default options when reading and decoding.
    let format_opts: FormatOptions = FormatOptions::default();
    let metadata_opts: MetadataOptions = MetadataOptions::default();
    let decoder_opts: DecoderOptions = DecoderOptions::default();

    // Probe the media source stream for a format.
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .unwrap();

    // Get the format reader yielded by the probe operation.
    let mut format = probed.format;

    // Get the default track.
    let track = format.default_track().unwrap();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .unwrap();

    // Store the track identifier, we'll use it to filter packets.
    let track_id = track.id;

    let mut sample_count = 0;
    let mut sample_buf = None;

    loop {
        // Get the next packet from the format reader.
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(e) => match e {
                symphonia::core::errors::Error::ResetRequired => {
                    info!("Assuming reset-required marks end-of-stream, this sample buf is now complete");
                    if let Some(sample_buf) = sample_buf {
                        return Ok(sample_buf);
                    }
                    panic!("Got reset-required, but no sample buf yet");
                }
                e => {
                    return Err(e);
                }
            },
        };

        // If the packet does not belong to the selected track, skip it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples, ignoring any decode errors.
        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                // The decoded audio samples may now be accessed via the audio buffer if per-channel
                // slices of samples in their native decoded format is desired. Use-cases where
                // the samples need to be accessed in an interleaved order or converted into
                // another sample format, or a byte buffer is required, are covered by copying the
                // audio buffer into a sample buffer or raw sample buffer, respectively. In the
                // example below, we will copy the audio buffer into a sample buffer in an
                // interleaved order while also converting to a f32 sample format.

                // If this is the *first* decoded packet, create a sample buffer matching the
                // decoded audio buffer format.
                if sample_buf.is_none() {
                    // Get the audio buffer specification.
                    let spec = *audio_buf.spec();

                    // Get the capacity of the decoded buffer. Note: This is capacity, not length!
                    let duration = audio_buf.capacity() as u64;

                    // Create the f32 sample buffer.
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                // Copy the decoded audio buffer into the sample buffer in an interleaved format.
                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(audio_buf);

                    // The samples may now be access via the `samples()` function.
                    sample_count += buf.samples().len();
                    debug!("\rDecoded {} samples", sample_count);
                }
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                panic!("Reset Error Encountered, something should be done but idk what");
            }
            Err(e) => {
                break Err(e);
            }
        }
    }
}

/// For each regular file in the given directory, if its an audio file,
/// it will be consolidated into a single resulting m4b file
/// that is written to the same directory. Each file will be its own chapter
///
/// # Errors
/// Will return an IO error if something goes wrong reading the files or their contents.
/// Any errors related to unrecognized/unsupported audio formats will be logged and
/// processing will continue.
///
pub fn process(p: &Path) -> Result<(), self::Error> {
    info!("Processing path: {}", p.display());
    let entries = std::fs::read_dir(p)?;

    for res in entries {
        let entry = res?;
        if let Ok(file_type) = entry.file_type() {
            if file_type.is_file() {
                info!("Found Regular file: {:?}", entry.path());
                let f = File::open(entry.path())?;
                match process_impl(f) {
                    Ok(()) => {
                        info!("Successfully processed file: {:?}", entry.path());
                    }
                    Err(e) => {
                        warn!(
                            "Could not process file: {:?} due to audio error: {:?}",
                            entry.path(),
                            e
                        );
                    }
                };
            }
        }
    }

    Ok(())
}

fn process_impl(f: File) -> Result<(), SymphoniaError> {
    let sample_buf = get_sample_buf(f)?;

    info!("Got sample buffer with {} samples", sample_buf.len());

    Ok(())
}
