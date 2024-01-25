# Pitch transposer v1.0

**Usage:** `pitch_transposer real-time phase-vocoder --scale <SCALE> --fsize <FSIZE> --hopa <HOPA>`  

Each indented block adds new parameters.  
`cargo run --bin pitch_transposer -- <TYPE> <ALGORITHM> [options]`  
  - TYPE: `static` or `real-time`
  - ALGORITHM: `phase-vocoder` or `basic`
  - [options]:  
          real-time : takes sound from the microphone  
                 phase_vocoder :  
                               --fsize <THE NUMBER OF SAMPLES PER FRAME>  
                               --hopa <THE GAP BETWEEN TO CONSECUTIVE FRAMES>  
         static : takes sound from a wav file  
                -i <PATH> : to the file to be transformed  
                -o <FILE_NAME> : the output file name  
*Examples :*  
  - `cargo run --bin pitch_transposer real-time phase-vocoder --fsize 1024 --hopa 256` transforms mic input in real time  
    - `cargo run --bin pitch_transposer real-time phase-vocoder --fsize 1024 -s 2.0 --hopa 256` transforms mic input in real time by the given scale factor  
  - `cargo run --bin pitch_transposer real-time phase-vocoder --in-path resources/mono_16PCM_440hz_8000sps.wav --out-filename transformed.wav --scale 2.4--fsize 1024 --hopa 256` transforms sound in the given file by multiplying frequencies by 2.4  

Output files are stored in `resources/outputs/`.  

# Quality checker v1.0

**Usage:** `cargo run --bin quality_checker -- --po <PATH_TO_ORACLE_SIGNAL> --ps <PATH_TO_SOURCE_SIGNAL> --th <THRESHOLD>`  
 - ps: the path to the signal to check  
 - po: the path to the "oracle" signal (i.e the reference)  
 - th: the threshold value of the distance between the oracle and the
        signal (a float number that holds on 32 bits)

*Example :*
        `cargo run --bin quality_checker -- --po 'resources/mono_16PCM_440hz_8000sps.wav' --ps 'resources/mono_16PCM_440hz_8000sps.wav' --th 3.0`  

The output is printed to standard output and it displays the following
elements separated by a blank space: `Good/Bad quality : euclidean distance`  

# Credit
Guillermo Morón Usón   
Sevi Dervishi
