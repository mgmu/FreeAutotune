use pitch;
use pitch::config::ptconfig::PitchCli;
use pitch::config::ptconfig::PitchSubcommand;
use pitch::audio::audio_clip::AudioClip;
use clap::Parser;
use cpal::traits::{HostTrait, DeviceTrait};

fn main() {

    let command = PitchCli::parse();
    match command.subcommand {
       PitchSubcommand::Static { static_subcommand } =>
            match pitch::transpose_pitch(static_subcommand) {
                Ok(()) => println!("Successfully transposed signal !"),
                Err(why) => println!("main() failed: {}", why)
            },
        PitchSubcommand::RealTime { real_time } => {
            // provides access to available audio devices on system
            let host = cpal::default_host();

            // input/ouput stream devices
            let idev = host.default_input_device().expect("no input found");
            let odev = host.default_input_device().expect("no output found");

            // input/ouput devices configuration
            let iconf = idev.default_input_config().expect("no conf found");
            let oconf = odev.default_output_config().expect("no conf found");

            // all of this in an infinite loop
            // record clip
            let len = 5;

            println!("Stop program with C-c");

            loop {
                let clip = AudioClip::record(&idev, iconf.clone(), len);

                // transpose clip
                let transposition_res = pitch::transpose_pitch_real_time(
                    &real_time,
                    clip.sample_rate as f32,
                    &clip.samples[..]
                );

                let data: Vec<f32> = match transposition_res {
                    Ok(data) => data,
                    Err(why) => panic!("main(): {}", why)
                };

                let transformed_clip = AudioClip {
                    sample_rate: clip.sample_rate,
                    samples: data
                };

                // playback clip
                println!("{}Listen...", AudioClip::STATUS);                
                transformed_clip.play(&odev, oconf.clone(), len);
            }
        }
    }
}
