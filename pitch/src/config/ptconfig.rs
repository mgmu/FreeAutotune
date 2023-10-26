use clap::{Parser,Subcommand};

#[derive(Parser)]
#[command(author, version = "v1.0")]
pub struct PitchCli {
    #[command(subcommand)]
    pub subcommand: PitchSubcommand
}
#[derive(Subcommand)]
pub enum  PitchSubcommand {
    Static {
        #[command(subcommand)]
        static_subcommand : PTConfiguration
    },

    RealTime {
        #[command(subcommand)]
        real_time : RealTimeSubCommand
    }
}

/// Configurations for static pitch transposers
#[derive(Subcommand)]
pub enum RealTimeSubCommand {
    Basic {
        
        ///the number of semi tons to shift. A negative value
        ///indicates shifting to a lower pitch, a positive value
        ///shifting to a higher pitch. 0 means no
        ///transformation.
        shift: i32,           // shift as semi tons
    },
    PhaseVocoder {
        
        
        /// the frame size to use
        #[clap(short,long)]
        fsize: usize,         // frame size
        
        ///the shift between frames
        #[arg(long)]          // must be long to not be mistaken with -h which stands for help
        hopa: usize,          // shift between frames

        /// Optional if not present autotune
        #[arg(long,short)]
        shift: Option<f32>
    },
}


/// Configurations for static pitch transposers
#[derive(Subcommand)]
pub enum PTConfiguration {
    Basic {
        /// the path to the .wav file to transform.
        #[arg(short,long)]   // short option  `-i` | long option `--in-path` 
        in_path: String,      // path to input file

        /// the name of the output file.
        #[arg(short,long)]   // short option  `-o` | long option `--out-filename` 
        out_filename: String, // ouput filename

        ///the number of semi tons to shift. A negative value
        ///indicates shifting to a lower pitch, a positive value
        ///shifting to a higher pitch. 0 means no
        ///transformation.
        shift: i32,           // shift as semi tons
    },
    PhaseVocoder {
        
        /// the path to the .wav file to transform.
        #[arg(short,long)]
        in_path: String,      // path to input file

        /// the name of the output file.
        #[arg(short,long)]
        out_filename: String, // ouput filename

        /// the frame size to use
        #[clap(short,long)]
        fsize: usize,         // frame size
        
        ///the shift between frames
        #[arg(long)]          // must be long to not be mistaken with -h which stands for help
        hopa: usize,          // shift between frames
    },
}



// Extracts the filename of the input path
// fn extract_input_filename(in_path: &str) -> &str {
//     let bytes = in_path.as_bytes();

//     for (i, &item) in bytes.iter().enumerate().rev() {
//         if item == b'/' {
//             return &in_path[i+1..in_path.len()];
//         }
//     }

//     &in_path[..]
// }

// produces the output filename for phase vocoder algorithm transformations
// fn phase_vocoder_output_filename_from_input(
//     name: &str,
//     scale: f32,
//     fsize: usize,
//     hopa: usize
// ) -> String {
//     let mut res = String::from("phase_vocoder");
//     res.push('_');
//     res.push_str(extract_input_filename(name));
//     res.push('_');
//     res.push_str(&scale.to_string());
//     res.push('_');
//     res.push_str(&fsize.to_string());
//     res.push('_');
//     res.push_str(&hopa.to_string());
//     res.push_str(".wav");
//     res
// }

// produces the output filename for basic algorithm transformations
// fn basic_output_filename_from_input(name: &str, shift: i32) -> String {
//     let mut res = String::from("basic");
//     res.push('_');
//     res.push_str(extract_input_filename(name));
//     res.push('_');
//     res.push_str(&shift.to_string());
//     res.push_str(".wav");
//     res
// }
