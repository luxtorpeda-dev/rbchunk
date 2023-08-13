extern crate rbchunk;
use std::env;
use std::process;

fn print_help() {
    println!(
        "Usage: rbchunk [-r] [-p (PSX)] [-w (wav)] [-s (swabaudio)]
         <image.bin> <image.cue> <basename>
Example: rbchunk foo.bin foo.cue foo
         rbchunk -ws foo.cue
  -r  Raw mode for MODE2/2352: write all 2352 bytes from offset 0 (VCD/MPEG)
  -p  PSX mode for MODE2/2352: write 2336 bytes from offset 24
      (default MODE2/2352 mode writes 2048 bytes from offset 24)
  -w  Output audio files in WAV format
  -s  swabaudio: swap byte order in audio tracks
    (try this if your audio comes up corrupted)"
    );
}

fn read_args() -> rbchunk::Args {
    let mut options: rbchunk::Args = Default::default();
        for arg in env::args().skip(1) {
            if arg.starts_with('-') {
                for c in arg.chars().skip(1) {
                    match c {
                        'r' => options.raw = true,
                        'p' => options.psx_truncate = true,
                        'v' => options.verbose = true,
                        'w' => options.to_wav = true,
                        's' => options.swap_audo_bytes = true,
                        _ => {
                            if c != 'h' {
                                eprintln!("Unknown flag: {}", c);
                            }
                            print_help();
                            process::exit(0);
                        }
                    }
                }
            } else if options.bin_file.is_empty() {
                options.bin_file = arg;
            } else if options.cue_file.is_empty() {
                options.cue_file = arg
            } else if options.output_name.is_empty() {
                options.output_name = arg;
            }
        }

        options
}

fn main() {
    println!(
        "rbchunk v2.0.0
https://github.com/luxtorpeda-dev/rbchunk
Based on bchunk by Heikki Hannikainen <hessu@hes.iki.fi>\n"
    );

    let args = env::args();
    if args.len() == 1 {
        print_help();
        process::exit(0);
    }

    let args = read_args();
    match rbchunk::convert(args) {
        Ok(()) => println!("Conversion complete!"),
        Err(err) => {
            println!("Error on conversion: {}", err);
            process::exit(1);
        }
    }
}
