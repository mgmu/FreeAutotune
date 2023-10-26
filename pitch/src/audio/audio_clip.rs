/// This code was adapted from https://github.com/RustAudio/cpal examples
use cpal::{Device, SupportedStreamConfig, FromSample, Sample};
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, StreamTrait};

/// An audio clip is a vector of audio samples sampled at a certain rate
#[derive(Clone)]
pub struct AudioClip {
    pub samples: Vec<f32>, // vector of temporal amplitudes
    pub sample_rate: u32, 
}

type ClipHandle = Arc<Mutex<Option<AudioClip>>>; // wrapper for shared clip

// Writes input device data into given audio clip. T is a Sample, but evrything
// is converted to f32 as AudioClip stores f32 temporal amplitude values. nbc
// indicates how many channels there are used by input device.
fn write_input_data<T>(input: &[T], nbc: u16, clipw: &ClipHandle)
where
    T: cpal::Sample, f32: FromSample<T>
{
    if let Ok(mut guard) = clipw.try_lock() {
        if let Some(clip) = guard.as_mut() { // encapsulated clip
            // iterate over chunks of nbc elements of input data
            for chunk in input.chunks(nbc as usize) {
                clip.samples.push(chunk[0].to_sample::<f32>()); // mono data
            }
        }
    }
}

type StateHandle = Arc<Mutex<Option<(usize, Vec<f32>)>>>;

// Writes input device data into given audio clip. T is a Sample, but everything
// is converted to f32 as AudioClip stores f32 temporal amplitude values. nbc
// indicates how many channels there are used by input device.
fn write_output_data(output: &mut [f32], nbc: u16, state: &StateHandle) {
    if let Ok(mut guard) = state.try_lock() {
        if let Some((i, data)) = guard.as_mut(){ // position and audio
            for chunk in output.chunks_mut(nbc.into()) {
                for sample in chunk.iter_mut() {
                     *sample = data
                     .get(*i)
                     .unwrap_or(&0f32)
                     .to_sample::<f32>();
                }
                *i += 1;           
            }
        }
    }
}

impl AudioClip {

    // Debug print starts with this constant
    pub const STATUS: &str = "[STATUS] ";

    /// Produces an audio clip from the output device of ~len seconds
    pub fn record(
        in_dev: &Device,
        in_conf: SupportedStreamConfig,
        len: u64
    ) -> AudioClip {
        let clip = AudioClip { // Where to store the input data
            samples: Vec::new(),
            sample_rate: in_conf.sample_rate().0 // the sample rate of input
        };

        // Allow clip to be multi-owned by using locks for access
        let clip = Arc::new(Mutex::new(Some(clip)));
        let clip2 = clip.clone(); // Used to write on clip

        // Callback function on stream error
        let err_fn = |err| eprintln!("error on stream: {}", err);

        // Number of input channels
        let nbc = in_conf.channels();

        // Create stream result depending on sample format
        let in_stream_res = match in_conf.sample_format() {
            cpal::SampleFormat::F32 => in_dev.build_input_stream(
                &in_conf.into(),
                move |data, _: &_| write_input_data::<f32>(data, nbc, &clip2),
                err_fn,
                None
            ),
            cpal::SampleFormat::I16 => in_dev.build_input_stream(
                &in_conf.into(),
                move |data, _: &_| write_input_data::<i16>(data, nbc, &clip2),
                err_fn,
                None
            ),
            cpal::SampleFormat::U16 => in_dev.build_input_stream(
                &in_conf.into(),
                move |data, _: &_| write_input_data::<u16>(data, nbc, &clip2),
                err_fn,
                None
            ),
            _ => todo!()
        };

        // Get input stream
        let stream = match in_stream_res {
            Ok(is) => is,
            Err(why) => panic!("record(): {}", why)
        };

        println!("{}Talk now", AudioClip::STATUS);

        // Start stream
        stream.play().unwrap();

        // Record for len seconds
        std::thread::sleep(std::time::Duration::from_secs(len));
        drop(stream);

        // Return recorded clip
        let clip = clip.lock().unwrap().take().unwrap();
        clip
    }

    pub fn play(
        &self, 
        out_dev: &Device, 
        out_conf: SupportedStreamConfig,
        len: u64
    ) {
        // Position in data and data to play
        let state = (0, self.samples.clone());
        let state = Arc::new(Mutex::new(Some(state)));

        // Callback function on stream error
        let err_fn = |err| eprintln!("error on stream: {}", err);

        // Number of output channelspp
        let nbc = out_conf.channels();

        // Create stream result depending on sample format
        let out_stream_res = match out_conf.sample_format() {
            _ => out_dev.build_output_stream(
                &out_conf.into(),
                move |data, _: &_| write_output_data(data, nbc, &state),
                err_fn,
                None
            )
        };

        // Get output stream
        let stream = match out_stream_res {
            Ok(is) => is,
            Err(why) => panic!("play(): {}", why)
        };

        // Start stream
        stream.play().unwrap();

        // Play back for len seconds
        std::thread::sleep(std::time::Duration::from_secs(len));
    }
}