for ((i = -12; i < 13; i++)); do cargo run --bin pitch_transposer -- 0 $1 $i 1024 256; done
