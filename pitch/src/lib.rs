use rustfft::{num_complex::Complex, FftPlanner};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::cmp::min;
use crate::config::ptconfig::PTConfiguration;
use crate::config::ptconfig::PTConfiguration::Basic;
use crate::config::ptconfig::PTConfiguration::PhaseVocoder;
use crate::config::qcconfig::QCConfiguration;
use crate::config::ptconfig::RealTimeSubCommand;

pub mod config;
pub mod convert;
pub mod audio;

pub fn transpose_pitch_real_time(
    config: &RealTimeSubCommand,
    samp_rate: f32,
    signal: &[f32]
) -> Result<Vec<f32>, &'static str> {
    let transposition = match config {
        RealTimeSubCommand::Basic { shift } => {
            let mut signal1 = vec![];
            signal1.extend_from_slice(&signal);
            basic_transposer(&signal1, *shift, samp_rate)
        },
        RealTimeSubCommand::PhaseVocoder { fsize, hopa, shift} => {
            let scale =  match shift {
                Some(s) => *s,
                None => match get_closest_scale_factor(signal, samp_rate) {
                    Some(sc) => sc,
                    None => return Err("could not find nearest note"),
                },
            };

            phase_vocoder_transposer(signal, *fsize, *hopa, samp_rate, scale)
        },
    };
    Ok(transposition)
}

pub fn get_closest_scale_factor(signal : &[f32], samp_rate: f32) -> Option<f32> {
    let mut planner = FftPlanner::new();
    let analyzed = apply_fft(&mut planner, &signal);
    let freq = get_main_frequency(&analyzed, samp_rate);
    let known_freq = known_frequencies();
    let closest_i = find_index_of_nearest_to(freq,
                                             0,
                                             known_freq.len(),
                                             &known_freq[..]);
    if closest_i == -1 {
        return None;
    }
    Some(freq / known_freq[closest_i as usize])
}

/// Transposes the pitch using the given configuration
pub fn transpose_pitch(config: PTConfiguration) -> Result<(), &'static str> {
    let path = match config {
        Basic { ref in_path, ..} => &in_path[..],
        PhaseVocoder { ref in_path, .. } => &in_path[..]
    };

    let (header, signal) = convert::extract_data_from_wav(path);
    let samp_rate: f32 = header.sampling_rate as f32;
    let transposition = match config {
        Basic {shift, ..} => basic_transposer(&signal, shift, samp_rate),
        PhaseVocoder {fsize, hopa, ..} => {
            
            let scale_factor = match get_closest_scale_factor(&signal, samp_rate) {
                Some(s) => s,
                None => return Err("could not find nearest note"),
            };
            phase_vocoder_transposer(&signal, fsize, hopa, samp_rate, scale_factor)
        },
    };

    // creating BitDepth acording to source file BitDepth
    let bit_depth = match header.bits_per_sample {
        8 =>
            wav::bit_depth::BitDepth::Eight(convert::to_u8(&transposition[..])),
        16 =>
            wav::bit_depth::BitDepth::Sixteen(
                convert::to_i16(&transposition[..])
            ),
        24 =>
            wav::bit_depth::BitDepth::TwentyFour(
                convert::to_i32(&transposition[..])
            ),
        _ => return Err("Unsupported codec"),
    };

    // write to file
    let filename = match config {
        Basic {
            ref out_filename, ..
        } => &out_filename[..],
        PhaseVocoder {
            ref out_filename, ..
        } => &out_filename[..],
    };

    let mut out_path = String::from("resources/outputs/");
    out_path.push_str(filename);
    let path = Path::new(&out_path);
    let mut writer = match File::create(path) {
        Ok(writer) => writer,
        _ => return Err("could not create file for transposition")
    };
    match wav::write(header, &bit_depth, &mut writer) {
        Ok(()) => Ok(()),
        Err(_) => Err("could not create file for transposition")
    }
}

/// Checks the quality using the given configuration
pub fn check_quality(
    config: QCConfiguration
) -> Result<(bool, f32), &'static str> {
    let path_signal = config.path_to_signal();
    let path_oracle = config.path_to_oracle();
    let (_, signal) = convert::extract_data_from_wav(path_signal);
    let (_, oracle) = convert::extract_data_from_wav(path_oracle);
    if signal.len() != oracle.len() {
        return Err("Can not compare signals of different length");
    }
    let mut planner = FftPlanner::new();
    let sspec = apply_fft(&mut planner, &signal[..]);
    let ospec = apply_fft(&mut planner, &oracle[..]);
    Ok(compute_and_compare_distance(config.threshold(), &sspec[..], &ospec[..]))
}

/// Shifts the given amplitudes in order to shift the corresponding frequencies
/// by the given shift value. For values that could not be shifted, 0 is
/// assigned.
fn shift_amplitudes(
    amplitudes: &[Complex<f32>],
    shift: i32,
    sampling_rate: f32,
) -> Vec<Complex<f32>> {
    let mut shifted = vec![Complex { re: 0.0, im: 0.0 }; amplitudes.len()];
    let len = amplitudes.len() as f32;
    let scale = 2.0f32.powf(shift as f32 / 12.0);
    let time_step = sampling_rate / len;
    let mut bin_freqs = vec![0.0; amplitudes.len()];
    for i in 0..bin_freqs.len() {
        bin_freqs[i] = bin_frequency(i as u32, sampling_rate, len as u32);
    }
    for i in 0..amplitudes.len() {
        let src_freq = i as f32 * sampling_rate / len / scale;
        if (src_freq / time_step).fract() == 0.0 && src_freq < sampling_rate {
            let i_src_freq = (src_freq * len / sampling_rate) as usize;
            shifted[i] = amplitudes[i_src_freq];
        } else {
            shifted[i] = Complex { re: 0.0, im: 0.0 };
        }
    }
    shifted
}

/// Implements the basic transposer, which basically transforms the signal to
/// the frequency domain, shifts the frequencies then transforms it back to the
/// time domain. Frequencies that do not have the amplitude in the FFT will
/// have their amplitudes determined by linear interpolation, using values
/// present in the FFT.
fn basic_transposer(
    signal: &[f32],
    shift: i32, sampling_rate: f32
) -> Vec<f32> {
    let mut planner = FftPlanner::new();
    let frequencies: Vec<Complex<f32>> = apply_fft(&mut planner, signal);
    let shifted = shift_amplitudes(&frequencies[..], shift, sampling_rate);
    let shifted_time_domain = apply_ifft(&mut planner, &shifted[..]);
    let mut reals_normalized = vec![0.0; shifted_time_domain.len()];
    let len = shifted_time_domain.len();
    for i in 0..len {
        reals_normalized[i] = shifted_time_domain[i].re / len as f32;
    }
    reals_normalized
}

/// Staticly transposes the `signal` by shifting it using the phase vocoder
/// algorithm
fn phase_vocoder_transposer(
    signal: &[f32],
    fsize: usize,
    hopa: usize,
    samp_rate: f32,
    scale_factor: f32
) -> Vec<f32> {
    let hops = (scale_factor * hopa as f32).round() as u32;
    let frames = frame(signal, fsize, hopa);
    let (frames, analyzed_frames) = parallelized_analysis(frames, hopa);
    let zero_frame = vec![Complex { re: 0.0, im: 0.0 }; fsize];
    let mut processed_frames = Vec::with_capacity(frames.len());

    for i in 0..analyzed_frames.len() {
        let curr_xa = &analyzed_frames[i];
        let prev_xa = if i > 0 {
            &analyzed_frames[i - 1]
        } else {
            &zero_frame
        };

        let prev_xp = if i > 0 {
            &processed_frames[i - 1]
        } else {
            &zero_frame
        };

        // process frame
        let mut curr_xp = vec![Complex { re: 0.0, im: 0.0 }; fsize];
        for k in 0..fsize {
            let freq_dev = frequency_deviation(
                prev_xa[k].arg(),
                curr_xa[k].arg(),
                hopa as u32,
                samp_rate,
                k as u32,
                fsize as u32,
            );
            let wrap_freq_dev = wrapped_frequency_deviation(freq_dev);
            let bin_freq = bin_frequency(k as u32, samp_rate, fsize as u32);
            let true_freq = true_frequency(wrap_freq_dev, bin_freq);
            let phi = if i != 0 {
                phase_adjustment(prev_xp[k].arg(), hops, samp_rate, true_freq)
            } else {
                analyzed_frames[0][k].arg()
            };
            curr_xp[k] = Complex::from_polar(analyzed_frames[i][k].norm(), phi);
        }
        processed_frames.push(curr_xp);
    }

    // synthetize frames
    let frames_for_oa = parallelized_synthesis(processed_frames, hops);

    // overlap-add frames
    let scaled_signal = overlap_add(&frames_for_oa[..], hops as usize);

    // resample scaled signal
    sample_audio(&scaled_signal[..], scale_factor)
}

/// Samples the given signal (audio) as if it was played scale_factor times
/// faster. If at some point it is not possible to take directly the amplitude
/// from the audio signal (if a sampling time is not whole for example), the
/// corresponding amplitude is deduced by linear interpolation. The scaling
/// factor should be of the form:
///     2^(t/12), where t corresponds to a number of semi-tons to shift.
fn sample_audio(signal: &[f32], scale_factor: f32) -> Vec<f32> {
    let len = signal.len();

    // computes the number of samples to take: signal_length / scale_factor
    let nb_samples = (len as f32 / scale_factor).round() as u32;

    // vector that will contain the amplitudes of the sampled signal
    let mut resampled_signal = Vec::with_capacity(nb_samples as usize);

    // sampling...
    for i in 0..nb_samples {
        // compute sample time point
        let sample_time = i as f32 * scale_factor;

        // if it is an integer, its amplitude can be accessed directly
        if sample_time.fract() == 0.0 {
            resampled_signal.push(signal[sample_time as usize]);

        // compute amplitude by linear interpolation
        } else {
            let x0 = sample_time.floor() as f32;
            let y0 = signal[x0 as usize];
            let tmp = sample_time.ceil() as f32;
            let x1 = if tmp >= len as f32 {
                (len - 1) as f32
            } else {
                tmp
            };
            let y1 = signal[x1 as usize];
            let amplitude = linear_interpolation(x0, y0, x1, y1, sample_time);
            resampled_signal.push(amplitude);
        }
    }
    resampled_signal
}

/// Computes f(x) by linear interpolation where f is a linear function
/// determined with the two-point form `(x0, y0)` and `(x1, y1)`.
fn linear_interpolation(x0: f32, y0: f32, x1: f32, y1: f32, x: f32) -> f32 {
    let rise = y1 - y0;
    let run = x1 - x0;
    let slope = rise / run;
    slope * (x - x0) + y0
}

/// Windows the given frame slice with the `von Hann window`
fn von_hann_window(frame: &[f32]) -> Vec<f32> {
    let mut res: Vec<f32> = vec![0.0; frame.len()];
    for i in 0..frame.len() {
        res[i] = frame[i] * von_hann(i, frame.len());
    }
    res
}

/// Creates `(signal.len() - frame_size) / hop_a + 1` frames of size
/// `frame_size` which values will range from
/// `signal[(i*hop_a)..(i*hop_a+frame_size)]` for the ith frame, where i ranges
/// from 0 to the number of frames, and stores them in a vector.
fn frame(signal: &[f32], frame_size: usize, hop_a: usize) -> Vec<Vec<f32>> {
    if frame_size > signal.len() {
        panic!("Provided frame size is greater than signal length!");
    }

    let nb_frames = (signal.len() - frame_size) / hop_a + 1;
    let mut frames: Vec<Vec<f32>> = Vec::with_capacity(nb_frames);
    for i in 0..nb_frames {
        frames.push(Vec::with_capacity(frame_size));
        let start = i * hop_a;
        let end = i * hop_a + frame_size;
        for j in start..end {
            frames[i].push(signal[j]);
        }
    }
    frames
}

/// Applies a forward FFT on the given array
fn apply_fft(
    planner: &mut FftPlanner<f32>,
    frame: &[f32]
) -> Vec<Complex<f32>> {
    let fft = planner.plan_fft_forward(frame.len());
    let mut buffer = convert::f32_to_complex_vector(&frame);
    fft.process(&mut buffer);
    buffer
}

/// Analyzes the frame by windowing it with a `von Hann` window, normalizing the
/// windowing and then transforming it with the FFT.
fn analyze_frame(
    mut planner: &mut FftPlanner<f32>,
    frame: &[f32],
    hopa: usize
) -> Vec<Complex<f32>> {
    let hann = von_hann_window(&frame);
    let norm: f32 = (frame.len() as f32 / hopa as f32 / 2.0).sqrt();
    let mut windowed = Vec::new();
    for i in 0..hann.len() {
        windowed.push(hann[i] / norm);
    }
    apply_fft(&mut planner, &windowed)
}

fn von_hann(x: usize, end: usize) -> f32 {
    let pi = std::f32::consts::PI;
    let two_pi = 2.0 * pi;
    let cos = (two_pi * x as f32 / end as f32).cos();
    0.5 - (0.5 * cos)
}

/// Creates a vector of length `frames[0].len() + (frames.len() - 1) * hops`
/// (it is supposed that `frames` is well-formed) which values correspond to
/// the overlap addition of `frames`. `hops` is not the "current" shift
/// between frames of `frames`, it is the product of the "current" shift
/// between frames and the scale factor.
/// Example:                hops = 4
///   |111111111|           = frames\[0\]
///     + |111111111|       = frames\[1\]
///         + |111111111|   = frames\[2\]
/// = |11112222322221111|   = output signal
fn overlap_add(frames: &[Vec<f32>], hops: usize) -> Vec<f32> {
    let flen = frames[0].len();
    let signal_length = flen + (frames.len() - 1) * hops;
    let mut signal: Vec<f32> = vec![0.0; signal_length];

    // overlap add
    for i in 0..frames.len() {
        for j in 0..flen {
            signal[j + i * hops] = signal[j + i * hops] + frames[i][j];
        }
    }
    signal
}

/// Computes the bin frequency at `bin_index`. The bin frequency is given by
/// the following formula:
///     `w_bin[k] = k * sampling_rate / frame_length`
fn bin_frequency(bin_index: u32, sampling_rate: f32, frame_length: u32) -> f32 {
    let pi = std::f32::consts::PI;
    let two_pi = 2.0 * pi;
    bin_index as f32 * sampling_rate / frame_length as f32 * two_pi
}

/// Computes the frequency deviation at `bin_index` between two consecutive
/// frames. The frequency deviation is computed as follows:
///     freq_dev = (<phi_curr - <phi_prev) / hopa_as_time - bin_frequency
/// The frequency deviation is the deviation that occurs when the frequency
/// phase at some bin k in the frame i-1 is different to the frequency phase at
/// the same bin k in the frame i.
fn frequency_deviation(
    phi_prev: f32,
    phi_curr: f32,
    hop_a: u32,
    sampling_rate: f32,
    bin_index: u32,
    frame_length: u32,
) -> f32 {
    let hopa_as_time = hop_a as f32 / sampling_rate;
    let bin_freq = bin_frequency(bin_index, sampling_rate, frame_length);
    (phi_curr - phi_prev) / hopa_as_time - bin_freq
}

/// Computes the wrapped frequency deviation at `bin_index` for two consecutive
/// frames. The wrapped frequency deviation is computed as follows:
///     wrap_freq_dev = ((freq_dev + PI) mod 2PI) - PI
fn wrapped_frequency_deviation(freq_dev: f32) -> f32 {
    let pi = std::f32::consts::PI;
    let two_pi = 2.0 * pi;
    ((freq_dev + pi) % (two_pi)) - pi
}

/// Computes the true frequency at `bin_index` for the frame i. The true
/// frequency is computed as follows:
///     true_freq = bin_frequency + wrapped_frequency_deviation
fn true_frequency(wrap_freq_dev: f32, bin_freq: f32) -> f32 {
    wrap_freq_dev + bin_freq
}

/// Computes the phase adjustment to avoid discontinuities between frame i-1
/// and frame i at `bin_index`. It is computed as follows:
///     phase_at_bin_index_frame_i =
///         phase_at_bin_index_frame_i-1 + hops_as_time * true_freq
fn phase_adjustment(
    previous_phase: f32,
    hops: u32,
    sampling_rate: f32,
    true_freq: f32
) -> f32 {
    let hops_as_time = hops as f32 / sampling_rate;
    previous_phase + hops_as_time * true_freq
}

/// Applies the inverse FFT on the given array
fn apply_ifft(
    planner: &mut FftPlanner<f32>,
    frame: &[Complex<f32>]
) -> Vec<Complex<f32>> {
    let fft = planner.plan_fft_inverse(frame.len());
    let mut buffer = Vec::from(frame);
    fft.process(&mut buffer);
    buffer
}

/// normalizes the given frame, i.e each element is scaled by 1/frame.len()
fn normalize(frame: &[f32]) -> Vec<f32> {
    let mut res: Vec<f32> = vec![0.0; frame.len()];
    let len = frame.len() as f32;
    for i in 0..len as usize {
        res[i] = frame[i] / len;
    }
    res
}

/// Given a vector of frames that are ready to be analyzed,
/// returns a vector containing the anlyzed frames.
fn parallelized_analysis(
    frames: Vec<Vec<f32>>,
    hopa: usize
) -> (Vec<Vec<f32>>, Vec<Vec<Complex<f32>>>) {
    let nb_of_frames = frames.len();
    let mut res: Vec<Vec<Complex<f32>>> = Vec::with_capacity(nb_of_frames);
    let mut handles = Vec::with_capacity(4);

    let closure = move |frames: &[Vec<f32>]| {
        let mut res: Vec<Vec<Complex<f32>>> = vec![];
        let mut planner: FftPlanner<f32> = FftPlanner::new();
        for frame in frames {
            let analyzed = analyze_frame(&mut planner, &frame, hopa);
            res.push(analyzed);
        }
        res
    };

    let frames_arc = Arc::new(frames);

    for i in 0..4 {
        let f = frames_arc.clone();
        let handle =
            thread::spawn(move || closure(
                &*&f[i * nb_of_frames / 4..(i + 1) * nb_of_frames / 4]
            ));
        handles.push(handle);
    }

    for handle in handles {
        let mut frame = handle.join().unwrap();
        res.append(&mut frame);
    }
    ((&(frames_arc.clone())).to_vec(), res)
}

/// Given a vector of frames that are ready for synthesis stage,
/// returns a vector containing the synthsized frames.
fn parallelized_synthesis(
    frames: Vec<Vec<Complex<f32>>>,
    hops: u32
) -> Vec<Vec<f32>> {
    let nb_of_frames = frames.len();
    let mut res: Vec<Vec<f32>> = Vec::with_capacity(nb_of_frames);
    let mut handles = Vec::with_capacity(4);

    let closure = move |frames: &[Vec<Complex<f32>>]| {
        let mut res: Vec<Vec<f32>> = vec![];
        let mut planner: FftPlanner<f32> = FftPlanner::new();
        for frame in frames {
            let xs_comp = apply_ifft(&mut planner, &frame[..]);
            let xs = reals_of(&xs_comp[..]);
            let normalized = normalize(&xs[..]);
            let hanned = von_hann_window(&normalized[..]);
            let mut windowed = Vec::new();
            let norm = (hanned.len() as f32 / hops as f32 / 2.0).sqrt();
            for i in 0..hanned.len() {
                windowed.push(hanned[i] / norm);
            }
            res.push(windowed);
        }
        res
    };

    let frames_arc = Arc::new(frames);

    for i in 0..4 {
        let f = frames_arc.clone();
        let handle =
            thread::spawn(move || closure(
                &*&f[i * nb_of_frames / 4..(i + 1) * nb_of_frames / 4]
            ));
        handles.push(handle);
    }

    for handle in handles {
        let mut frame = handle.join().unwrap();
        res.append(&mut frame);
    }
    res
}

/// Returns a `Vec<f32>` containing the real part of each `Complex<f32>` in
/// `frame`
fn reals_of(frame: &[Complex<f32>]) -> Vec<f32> {
    let mut res: Vec<f32> = Vec::with_capacity(frame.len());
    for elt in frame {
        res.push(elt.re);
    }
    res
}

/// Computes the norm of `v`, a vector of Rn and returns it. The formula used
/// for computation is : norm = sqrt(sum from i = 0 to n of squared(vi))
fn norm(v: &[f32]) -> f32 {
    let mut sum_of_squares = 0.0;
    for i in 0..v.len() { sum_of_squares += v[i].powf(2.0); }
    sum_of_squares.sqrt()
}

/// Computes the Euclidian distance between `v1` and `v2`, two vectors of Rn,
/// where `n = min(v1.len(), v2.len())`. The Euclidian distance is computed
/// according to this formula:  
///    2-distance = sqrt(sum(squared(v1\[i\] - v2\[i\]), i in 0..n))
fn euclidian_distance(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() == 0 { return norm(v2); }
    if v2.len() == 0 { return norm(v1); }
    let n = min(v1.len(), v2.len());
    let mut sum_of_squares = 0.0;
    for i in 0..n {
        let diff: f32 = v1[i] - v2[i];
        sum_of_squares += diff.powf(2.0);
    }
    sum_of_squares.sqrt()
}

/// Computes the euclidian distance between the spectrums of `sig1` and `sig2`
/// and returns a tuple containing the result of the test and the distance
fn compute_and_compare_distance(
    epsilon: f32,
    sig1: &[Complex<f32>],
    sig2: &[Complex<f32>]
) -> (bool, f32) {
    let v1 = reals_of(sig1);
    let v2 = reals_of(sig2);
    let dist = euclidian_distance(&v1, &v2);
    (dist <= epsilon, dist)
}

/// Based on the index of the maximum amplitude of the complex numbers of v
/// (not counting complex number at index 0 of v), returns the corresponding bin
/// frequency.
fn get_main_frequency(v: &[Complex<f32>], samp_rate: f32) -> f32 {
    let mut imax = -1;
    let mut max = f32::NEG_INFINITY;
    for i in 1..v.len() {
        if v[i].re > max {
            imax = i as i32;
            max = v[i].re;
        }
    }
    if imax == -1 {
        0.0
    } else {
        bin_frequency(imax as u32, samp_rate, v.len() as u32)
    }
}

/// Returns the index of the nearest value to f in v. Returns -1 if v is of
/// length 0 or if r is 0 or r is inferior or equal to l.
fn find_index_of_nearest_to(f: f32, l: usize, r: usize, v: &[f32]) -> i32 {
    if v.len() == 0 || r == 0 || r < l {
        return -1;
    }

    if r - l <= 1 {
        if r == v.len() {
            return (r - 1) as i32;
        } else {
            let dist_lf = (f - v[l]).abs();
            let dist_rf = (f - v[r]).abs();
            if dist_lf <= dist_rf {
                return l as i32;
            } else {
                return r as i32;
            }
        }
    }
    let middle = (l + r) / 2;
    if v[middle] == f {
        middle as i32
    } else if v[middle] < f {
        find_index_of_nearest_to(f, middle + 1, r, v)
    } else {
        find_index_of_nearest_to(f, l, middle, v)
    }
}

fn known_frequencies() -> Vec<f32> {
    vec![
        16.35, 17.32, 18.35, 19.45, 20.60, 21.83, 23.12, 24.50, 25.96, 27.50,
        29.14, 30.87, 32.70, 34.65, 36.71, 38.89, 41.20, 43.65, 46.25, 49.00,
        51.91, 55.00, 58.27, 61.74, 65.41, 69.30, 73.42, 77.78, 82.41, 87.31,
        92.50, 98.00, 103.83, 110.00, 116.54, 123.47, 130.81, 138.59, 146.83,
        155.56, 164.81, 174.61, 185.00, 196.00, 207.65, 220.00, 233.08, 246.94,
        261.63, 277.18, 293.66, 311.13, 329.63, 349.23, 369.99, 392.00, 415.30,
        440.00, 466.16, 493.88, 523.25, 554.37, 587.33, 622.25, 659.26, 698.46,
        739.99, 783.99, 830.61, 880.00, 932.33, 987.77, 1046.50, 1108.73,
        1174.66, 1244.51, 1318.51, 1396.91, 1479.98, 1567.98, 1661.22, 1760.00,
        1864.66, 1975.53, 2093.00, 2217.46, 2349.32, 2489.02, 2637.02, 2793.83,
        2959.96, 3135.96, 3322.44, 3520.00, 3729.31, 3951.07, 4186.01, 4434.92,
        4698.64, 4978.03, 5274.04, 5587.65, 5919.91, 6271.93, 6644.88, 7040.00,
        7458.62, 7902.13
    ]
}

#[cfg(test)]
mod lib_tests {
    use super::*;

    /// Rounds `f` to `r` decimal digits
    fn round_digits(f: f32, r: i32) -> f32 {
        (f * 10.0_f32.powi(r)).round() / 10.0_f32.powi(r)
    }

    #[test]
    fn test_sample_audio() {
        let signal_sa = vec![1.5, 1.0, 0.5, 1.75, 2.0, 3.0, 2.5, 1.5, 0.25];

        let sf1 = 2.0;
        let mut t1 = sample_audio(&signal_sa, sf1);
        for i in 0..5 {
            t1[i] = round_digits(t1[i], 2);
        }
        assert_eq!(t1, [1.5, 0.5, 2.0, 2.5, 0.25]);

        let sf2 = 1.5;
        let mut t2 = sample_audio(&signal_sa, sf2);
        for i in 0..6 {
            t2[i] = round_digits(t2[i], 3)
        }
        assert_eq!(t2, [1.5, 0.75, 1.75, 2.5, 2.5, 0.875]);

        let sf3 = 0.8;
        let mut t3 = sample_audio(&signal_sa, sf3);
        for i in 0..11 {
            t3[i] = round_digits(t3[i], 2);
        }
        assert_eq!(
            t3,
            [1.5, 1.1, 0.7, 1.0, 1.8, 2.0, 2.8, 2.7, 2.1, 1.25, 0.25]
        );

        let sf4 = -0.1;
        let t4 = sample_audio(&signal_sa, sf4);
        assert!(t4.is_empty());
    }

    #[test]
    fn test_frame() {
        let signal = vec![1.2, 4.7, 2.9, 3.2, 5.9, 6.1, 0.4, 2.2];
        let frames: Vec<Vec<f32>> = frame(&signal, 3, 2);
        assert!(frames.len() == 3);
        assert_eq!(frames[0], [1.2, 4.7, 2.9]);
        assert_eq!(frames[1], [2.9, 3.2, 5.9]);
        assert_eq!(frames[2], [5.9, 6.1, 0.4]);

        let signal = vec![1.2, 4.7, 2.9, 3.2, 5.9, 6.1, 0.4, 2.2, 19.4];
        let frames: Vec<Vec<f32>> = frame(&signal, 3, 2);
        assert!(frames.len() == 4);
        assert_eq!(frames[0], [1.2, 4.7, 2.9]);
        assert_eq!(frames[1], [2.9, 3.2, 5.9]);
        assert_eq!(frames[2], [5.9, 6.1, 0.4]);
        assert_eq!(frames[3], [0.4, 2.2, 19.4]);
    }

    #[test]
    fn test_von_hann() {
        let expected: f32 = 1.0;
        let res = von_hann(1, 2);
        assert_eq!(res, expected);

        let expected: f32 = 0.0;
        let res = von_hann(0, 3);
        assert_eq!(res, expected);

        let expected: f32 = 0.75;
        let res = von_hann(1, 3);
        assert_eq!(res, expected);
    }

    #[test]
    fn test_von_hann_window() {
        let frame1 = vec![2.5, 5.8, 7.78];
        let mut windowed_frame = von_hann_window(&frame1);
        // needs rounding
        for i in 0..3 {
            windowed_frame[i] = round_digits(windowed_frame[i], 3);
        }
        let expected: Vec<f32> = vec![0.0, 4.35, 5.835];
        assert_eq!(windowed_frame, expected);
    }

    #[test]
    fn test_overlap_add() {
        let input = vec![
            vec![3.4, 5.7, 2.8],
            vec![1.2, 3.1, 2.4],
            vec![-4.1, 0.9, 1.4],
        ];
        let mut output = overlap_add(&input[..], 1);
        // needs rounding
        for i in 0..5 {
            output[i] = round_digits(output[i], 1);
        }
        assert_eq!(output, [3.4, 6.9, 1.8, 3.3, 1.4]);
        let output = overlap_add(&input[..], 3);
        assert_eq!(output, [3.4, 5.7, 2.8, 1.2, 3.1, 2.4, -4.1, 0.9, 1.4]);
    }

    #[test]
    fn test_frequency_deviation() {
        let p: Complex<f32> = Complex::new(3.5, 2.0);
        let c: Complex<f32> = Complex::new(2.9, 0.5);
        let dev = frequency_deviation(p.arg(), c.arg(), 256, 48000.0, 0, 1024);
        assert_eq!(round_digits(dev, 4), -65.3271);
    }

    #[test]
    fn test_normalize() {
        let input = vec![2.0, 2.0];
        let normalized_vec = normalize(&input[..]);
        assert_eq!(normalized_vec, vec![1.0, 1.0]);
    }

    #[test]
    fn shift_amplitudes_by_12() {
        let amplitudes = vec![
            Complex { re: 1.0, im: 2.0 },
            Complex { re: 3.0, im: 4.0 },
            Complex { re: 5.0, im: 6.0 },
            Complex { re: 7.0, im: 8.0 },
            Complex { re: 10.0, im: 9.0 },
            Complex { re: 12.0, im: 11.0 },
            Complex { re: 14.0, im: 13.0 },
            Complex { re: 16.0, im: 15.0 },
            Complex { re: 18.0, im: 17.0 },
            Complex { re: 20.0, im: 19.0 },
        ];
        let shift = 12;
        let sampling_rate = 8000.0;
        let res = shift_amplitudes(&amplitudes[..], shift, sampling_rate);
        let expected = vec![
            Complex { re: 1.0, im: 2.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 3.0, im: 4.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 5.0, im: 6.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 7.0, im: 8.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 10.0, im: 9.0 },
            Complex { re: 0.0, im: 0.0 },
        ];
        assert_eq!(expected, res);
    }

    #[test]
    fn shift_amplitudes_by_neg_12() {
        let amplitudes = vec![
            Complex { re: 1.0, im: 2.0 },
            Complex { re: 3.0, im: 4.0 },
            Complex { re: 5.0, im: 6.0 },
            Complex { re: 7.0, im: 8.0 },
            Complex { re: 10.0, im: 9.0 },
            Complex { re: 12.0, im: 11.0 },
            Complex { re: 14.0, im: 13.0 },
            Complex { re: 16.0, im: 15.0 },
            Complex { re: 18.0, im: 17.0 },
            Complex { re: 20.0, im: 19.0 },
        ];
        let shift = -12;
        let sampling_rate = 8000.0;
        let res = shift_amplitudes(&amplitudes[..], shift, sampling_rate);
        let expected = vec![
            Complex { re: 1.0, im: 2.0 },   // X0
            Complex { re: 5.0, im: 6.0 },   // X2
            Complex { re: 10.0, im: 9.0 },  // X4
            Complex { re: 14.0, im: 13.0 }, // X6
            Complex { re: 18.0, im: 17.0 }, // X8
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
            Complex { re: 0.0, im: 0.0 },
        ];
        assert_eq!(expected, res);
    }

    #[test]
    fn shift_amplitudes_by_0() {
        let amplitudes = vec![
            Complex { re: 1.0, im: 2.0 },
            Complex { re: 3.0, im: 4.0 },
            Complex { re: 5.0, im: 6.0 },
            Complex { re: 7.0, im: 8.0 },
            Complex { re: 10.0, im: 9.0 },
            Complex { re: 12.0, im: 11.0 },
            Complex { re: 14.0, im: 13.0 },
            Complex { re: 16.0, im: 15.0 },
            Complex { re: 18.0, im: 17.0 },
            Complex { re: 20.0, im: 19.0 },
        ];
        let shift = 0;
        let sampling_rate = 8000.0;
        let res = shift_amplitudes(&amplitudes[..], shift, sampling_rate);
        let expected = vec![
            Complex { re: 1.0, im: 2.0 },
            Complex { re: 3.0, im: 4.0 },
            Complex { re: 5.0, im: 6.0 },
            Complex { re: 7.0, im: 8.0 },
            Complex { re: 10.0, im: 9.0 },
            Complex { re: 12.0, im: 11.0 },
            Complex { re: 14.0, im: 13.0 },
            Complex { re: 16.0, im: 15.0 },
            Complex { re: 18.0, im: 17.0 },
            Complex { re: 20.0, im: 19.0 },
        ];
        assert_eq!(expected, res);
    }

    #[test]
    fn test_keep_real_part() {
        let complex_vec = vec![
            Complex::new(32.42, 15.798),
            Complex::new(6876.4, 189.989)
        ];
        let expected = vec![32.42, 6876.4];
        let result = super::reals_of(&complex_vec[..]);
        assert_eq!(expected, result);
    }

    #[test]
    fn euclidian_distance_of_zero_vectors_is_zero() {
        let zeros = vec![0.0; 10];
        assert_eq!(0.0, euclidian_distance(&zeros[..], &zeros[..]));
    }

    #[test]
    fn euclidian_distance_collinear_vectors_is_difference_of_nth_component() {
        let v = vec![4.3, 5.6, 9.2, 10.0];
        let u = vec![4.3, 5.6, 9.2, 9.0];
        assert_eq!(1.0, euclidian_distance(&v[..], &u[..]));
    }

    #[test]
    fn euclidian_distance_of_li_vectors_is_pythagoras_theorem() {
        let v = vec![1.0, 1.0];
        let u = vec![3.0, 1.0];
        assert_eq!(2.0, euclidian_distance(&v[..], &u[..]));
    }

    #[test]
    fn euclidian_distance_zero_vector_and_non_zero_vector_is_norm_of_second() {
        let v: Vec<f32> = vec![];
        let u = vec![3.0, 0.0];
        assert_eq!(3.0, euclidian_distance(&v[..], &u[..]));
    }

    #[test]
    fn norm_of_dim_2_vector_is_length_of_hypothenuse() {
        let v = vec![3.0, 4.0];
        assert_eq!(5.0, norm(&v[..]));
    }

    #[test]
    fn norm_of_null_vector_is_zero() {
        let v = vec![0.0; 100];
        assert_eq!(0.0, norm(&v[..]));
    }

    #[test]
    fn find_in_empty_vec_returns_neg_1() {
        let v = Vec::new();
        assert_eq!(-1, find_index_of_nearest_to(4.2, 0, v.len(), &v[..]));
    }

    #[test]
    fn find_with_d_inferior_or_equal_to_0_returns_neg_1() {
        let v = vec![0.0];
        assert_eq!(-1, find_index_of_nearest_to(4.2, 0, 0, &v[..]));
    }

    #[test]
    fn find_with_d_inferior_or_equal_to_g_returns_neg_1() {
        let v = vec![4.3, 5.8];
        assert_eq!(-1, find_index_of_nearest_to(4.2, 4, 3, &v[..]));
    }

    #[test]
    fn find_returns_index_of_nearest() {
        let v = vec![4.2, 5.3, 7.2, 9.1, 11.0];
        assert_eq!(1, find_index_of_nearest_to(4.76, 0, v.len(), &v[..]));
    }

    #[test]
    fn find_returns_index_of_nearest2() {
        let v = vec![4.2, 5.3, 7.2, 9.1, 11.0];
        assert_eq!(0, find_index_of_nearest_to(4.75, 0, v.len(), &v[..]));
    }

    #[test]
    fn find_returns_index_of_nearest3() {
        let v = vec![4.2, 5.3, 7.2, 9.1, 11.0];
        assert_eq!(0, find_index_of_nearest_to(4.2, 0, v.len(), &v[..]));
    }

    #[test]
    fn find_440_returns_index_of_value_440() {
        let notes = known_frequencies();
        assert_eq!(57, find_index_of_nearest_to(440.00, 0, notes.len(), &notes[..]));
    }

    #[test]
    fn find_439_05_returns_index_of_value_440() {
        let notes = known_frequencies();
        assert_eq!(57, find_index_of_nearest_to(439.05, 0, notes.len(), &notes[..]));
    }

    #[test]
    fn find_34_539944() {
        let notes = known_frequencies();
        assert_eq!(13, find_index_of_nearest_to(34.539944, 0, notes.len(), &notes[..]));
    }
}
