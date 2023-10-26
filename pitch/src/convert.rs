use file_format::FileFormat;
use wav::BitDepth;
use std::fs::File;
use std::path::Path;
use num_complex::Complex;

/// Returns true if the given path is a .wav file.
fn is_wav_file(path: &str) -> bool {
    match FileFormat::from_file(path) {
        Ok(format) => format == FileFormat::WaveformAudio,
        Err(why) => panic!("is_wav_file(): {}", why),
    }
}

/// Returns a File corresponding to the file at the given path if is
/// a .wav file.
fn open_wav_file(path: &str) -> File {
    if !is_wav_file(path) {
        panic!("open_wav_file(): Not a .wav file");
    }

    match File::open(&Path::new(path)) {
        Ok(file) => file,
        Err(why) => panic!("open_wav_file(): {}", why),
    }
}

/// Extracts the audio data from the given .wav file. If data can not be
/// converted to `Vec<f32>` an empty vector is returned.
pub fn extract_data_from_file(
    mut file: &File
) -> (wav::header::Header, Vec<f32>) {
    let (header, data) = if let Ok((header, data)) = wav::read(&mut file) {
        (header, data)
    } else {
        panic!("Error!");
    };

    match data {
        BitDepth::Sixteen(res) => (header, i16_to_f32_vector(res)),
        BitDepth::TwentyFour(res) => (header, i32_to_f32_vector(res)),
        BitDepth::ThirtyTwoFloat(res) => (header, res),
        _ => (header, vec![] as Vec<f32>),
    }
}

/// extracts data from the given path.
/// the function panics in case open_wav_file or extract_data_from_file calls
/// panic.
pub fn extract_data_from_wav(path: &str) -> (wav::header::Header, Vec<f32>) {
    let file = open_wav_file(path);
    extract_data_from_file(&file)
}

/// converts the given i16 vector to an f32 vector.
fn i16_to_f32_vector(to_convert: Vec<i16>) -> Vec<f32> {
    let mut res: Vec<f32> = Vec::with_capacity(to_convert.len());
    for elt in to_convert {
        res.push(elt as f32);
    }
    res
}

/// converts the given i32 vector to an f32 vector.
fn i32_to_f32_vector(to_convert: Vec<i32>) -> Vec<f32> {
    let mut res: Vec<f32> = Vec::with_capacity(to_convert.len());
    for elt in to_convert {
        res.push(elt as f32);
    }
    res
}

pub fn to_u8(to_convert: &[f32]) -> Vec<u8> {
    let mut res: Vec<u8> = Vec::with_capacity(to_convert.len());
    for i in 0..to_convert.len() {
        res.push(to_convert[i] as u8);
    }
    res
}

pub fn to_i16(to_convert: &[f32]) -> Vec<i16> {
    let mut res: Vec<i16> = Vec::with_capacity(to_convert.len());
    for i in 0..to_convert.len() {
        res.push(to_convert[i] as i16);
    }
    res
}

pub fn to_i32(to_convert: &[f32]) -> Vec<i32> {
    let mut res: Vec<i32> = Vec::with_capacity(to_convert.len());
    for i in 0..to_convert.len() {
        res.push(to_convert[i] as i32);
    }
    res
}

/// Given the channels of a stereo track (they must be of same length), returns
/// a vector containing the mono conversion, by computing the mean of the two
/// channels at each point of time.
// fn stereo_to_mono(ch1: &[f32], ch2: &[f32]) -> Vec<f32> {
//     let len = ch1.len();

//     if len != ch2.len() {
//         panic!("stereo_to_mono() failed: Invalid channel lengths");
//     }

//     let mut mono: Vec<f32> = Vec::with_capacity(len);
//     for i in 0..len {
//         mono.push((ch1[i] + ch2[i]) / 2.0);
//     }
//     mono
// }

/// converts the given `f32` vector to a `Complex<f32>` vector.
pub fn f32_to_complex_vector(to_convert: &[f32]) -> Vec<Complex<f32>> {
    let mut res: Vec<Complex<f32>> = Vec::with_capacity(to_convert.len());
    for i in 0..to_convert.len() {
        res.push(Complex::new(to_convert[i], 0.0));
    }
    res
}

#[cfg(test)]
mod convert_tests {
    use super::*;

    #[test]
    fn test_wav_reader() {
        let wav_file_path = String::from(
            "resources/mono_16PCM_440hz_8000sps.wav"
        );
        let text_file_path = String::from("resources/not_a_wav_file.txt");
        assert!(is_wav_file(&wav_file_path));
        assert!(!is_wav_file(&text_file_path));
    }

    #[test]
    fn test_f32_to_complex_vector() {
        let res = f32_to_complex_vector(&vec![2.4, 4.64, 5.68]);
        assert_eq!(
            res,
            vec![
                Complex::new(2.4, 0.0),
                Complex::new(4.64, 0.0),
                Complex::new(5.68, 0.0)
            ]
        );
    }

    /*#[test]
    fn test_stereo_to_mono() {
        let ch1 = [54.0, -13.0, 15.0, 19.0];
        let ch2 = [13.0, 9.0, 34.0, 0.0];
        let expected = vec![33.5, -2.0, 24.5, 9.5];
        let result = super::stereo_to_mono(&ch1, &ch2);
        assert_eq!(expected, result);
    }*/
}
