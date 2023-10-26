#!/usr/bin/bash

FINAL_FILE_NAME="mono_16PCM_"
#"mono_16PCM_440hz_8000sps.wav"

if (($# != 3))
then
    echo "$0 <sine frequency in Hz> <sample rate in Hz> <duration in seconds>"
exit 1
fi
FINAL_FILE_NAME="${FINAL_FILE_NAME}${1}hz_${2}sps.wav"
echo $FINAL_FILE_NAME

ffmpeg -f lavfi -i "sine=frequency=$1:sample_rate=$2:duration=$3" -c:a pcm_s16le $FINAL_FILE_NAME
