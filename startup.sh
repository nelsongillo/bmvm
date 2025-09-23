#!/bin/bash

set -e

# Usage
USAGE="Usage: $0 -b <benchy> -f <directory> -o <output> -m <bmvm|wasm|native> -w <warmups> -i <iterations> -l <links>"

# Default values
benchy="./target/release/benchy"
input=""
output="."
runtime=""
warmup="0"
iters="10"
dry=false
linkings="1"

is_number='^[0-9]+$'

# arg parse
while getopts 'b:f:o:m:w:i:l:dh' opt; do
    case "$opt" in
        b)
            if [[ ! -x "$OPTARG" ]]; then
                echo "Error: Benchy '$OPTARG' not found."
                exit 1
            fi
            benchy="$OPTARG"
            ;;
        f)
            if [[ ! -d "$OPTARG" ]]; then
                echo "Error: Input directory '$OPTARG' not found."
                exit 1
            fi
            input="$OPTARG"
            ;;
        o)
            output="$OPTARG"
            ;;
        m)
            if [[ "$OPTARG" != "bmvm" && "$OPTARG" != "wasm" && "$OPTARG" != "native" ]]; then
                echo "Error: Invalid mode '$OPTARG'. Must be 'bmvm', 'wasm' or 'native'."
                exit 1
            fi
            runtime="$OPTARG"
            ;;
        w)
            if ! [[ $OPTARG =~ $is_number ]] ; then
               echo "Error: Warmup must be a number"
               exit 1
            fi
            warmup="$OPTARG"
            ;;
        i)
            if ! [[ $OPTARG =~ $is_number ]] ; then
               echo "Error: Iterations must be a number"
               exit 1
            fi
            iters="$OPTARG"
            ;;
        l)
            if ! [[ $OPTARG =~ $is_number ]] ; then
               echo "Error: Links must be a number"
               exit 1
            fi
            linkings="$OPTARG"
            ;;
        d)
            dry=true
            ;;
        ?)
            echo "$USAGE"
            exit 1
            ;;
    esac
done

# Validate arguments
if [[ -z "$input" || -z "$runtime" || -z "$warmup" || -z "$iters" ]]; then
    echo "$USAGE"
    exit 1
fi



# build benchy
if $dry; then
    echo "cargo build --release --package benchy --features links$linkings"
else
    cargo build --release --package benchy --features="links$linkings"
fi


# Find files based on mode
FILES=()
if [[ "$runtime" == "wasm" ]]; then
        FILES+=("$input/link-wasm-links$linkings.wasm")
elif [[ "$runtime" == "bmvm" ]]; then
    # Find all executable files
        FILES+=("$input/link-bmvm-links$linkings")
fi

# Call foo for each file
for file in "${FILES[@]}"; do
    if $dry; then
        echo "$benchy --warmup $warmup --iters $iters --runtime $runtime --file $file --output $output --mode start"
    else
        $benchy --warmup "$warmup" --iters "$iters" --runtime "$runtime" --file "$file" --output "$output" --mode "start"
    fi
done