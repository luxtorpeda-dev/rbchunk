use std::env;
use std::fmt::Display;
use std::fs;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::mem::swap;
use std::ops::IndexMut;
use std::process;

const SECTOR_SIZE: u64 = 2352;

#[derive(Default)]
pub struct Args {
    output_name: String,
    bin_file: String,
    cue_file: String,
    verbose: bool,
    psx_truncate: bool,
    raw: bool,
    swap_audo_bytes: bool,
    to_wav: bool
}

impl Args {
    pub fn new() -> Self {
        let mut options: Args = Default::default();
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
        /*
         * If binfile is not supplied we can read it from CUE file
         * This could have been done in a better way, but for the sake of
         * compatibility with the original program we have to do it this way
         */
        if options.bin_file.is_empty() {
            eprintln!("CUE file is missing!");
            print_help();
            process::exit(1)
        } else if options.cue_file.is_empty() {
            swap(&mut options.cue_file, &mut options.bin_file);
        }

        if options.output_name.is_empty() {
            options.output_name = String::from(
                options
                    // Get filename without extension
                    .cue_file
                    .split('/')
                    .next_back()
                    .unwrap()
                    .split('.')
                    .next()
                    .unwrap(),
            );
        }

        options
    }
}

#[derive(Default)]
pub struct Track {
    start_sector: u64,
    stop_sector: Option<u64>,
    start: u64,
    stop: Option<u64>,
    mode: Mode,
    extension: Extension,
    number: u32,
    audio: bool,
    data_block_offset: u32,
    data_block_size: u32,
}

impl Track {
    fn get_track_mode(&mut self, a: &Args) {
        match self.mode {
            Mode::Unknown => {
                self.data_block_offset = 0;
                self.data_block_size = 2352;
                self.extension = Extension::Ugh;
            }
            Mode::Audio => {
                self.data_block_offset = 0;
                self.data_block_size = 2352;
                self.audio = true;
                if a.to_wav {
                    self.extension = Extension::Wav;
                } else {
                    self.extension = Extension::Cdr;
                }
            }
            Mode::Mode1_2352 => {
                self.data_block_offset = 16;
                self.data_block_size = 2048;
                self.extension = Extension::Iso;
            }
            Mode::Mode2_2352 => {
                self.extension = Extension::Iso;
                if a.raw {
                    self.data_block_offset = 0;
                    self.data_block_size = 2352;
                } else if a.psx_truncate {
                    self.data_block_offset = 0;
                    self.data_block_size = 2336;
                } else {
                    self.data_block_offset = 24;
                    self.data_block_size = 2048;
                }
            }
            Mode::Mode2_2336 => {
                self.data_block_offset = 16;
                self.data_block_size = 2336;
                self.extension = Extension::Iso;
            }
        }
    }

    fn wav_header(&self) -> Vec<u8> {
        // Constructing wav header in vector so that we can write it in a single write
        const WAV_FORMAT_HLEN: u64 = 24;
        const WAV_DATA_HLEN: u64 = 8;
        let reallen =
            (self.stop_sector.unwrap() - self.start_sector + 1) * self.data_block_size as u64;

        let wav_header = [
            // RIFF header
            "RIFF".as_bytes(),
            ((reallen + WAV_DATA_HLEN + WAV_FORMAT_HLEN + 4) as u32)
                .to_le_bytes()
                .as_slice(), // length of file starting from WAVE
            "WAVE".as_bytes(),
            // FORMAT HEADER
            "fmt ".as_bytes(),
            0x10_u32.to_le_bytes().as_slice(), // length of FORMAT header
            0x1_u16.to_le_bytes().as_slice(),  // constant
            0x2_u16.to_le_bytes().as_slice(),  //channels
            44100_u32.to_le_bytes().as_slice(), // sample rate
            (44100_u32 * 4).to_le_bytes().as_slice(), // bytes per second
            0x4_u16.to_le_bytes().as_slice(),  // bytes per sample
            0x10_u16.to_le_bytes().as_slice(), // bits per channel,
            //DATA header
            "data".as_bytes(),
            (reallen as u32).to_le_bytes().as_slice(),
        ]
        .concat();
        wav_header
    }

    fn write_to_file(&self, reader: &mut BufReader<&std::fs::File>, a: &Args) {
        let filename = format!(
            "{}{:0>2}.{}",
            a.output_name,
            self.number,
            self.extension.as_ref()
        );
        let sectors = self.stop_sector.unwrap() - self.start_sector + 1;
        let file_length = sectors * self.data_block_size as u64;
        let mut sector = [0u8; SECTOR_SIZE as usize];

        let out_file = match fs::File::create(&filename) {
            Ok(t_file) => t_file,
            Err(e) => {
                eprintln!("Could not write to track:\n{}", e);
                process::exit(4);
            }
        };

        let mut writer: std::io::BufWriter<&std::fs::File> =
            std::io::BufWriter::with_capacity(SECTOR_SIZE as usize * 16, &out_file);

        if let Err(e) = reader.seek(SeekFrom::Start(self.start)) {
            eprintln!("Could not seek to track location\n{}", e);
            process::exit(4);
        }

        if a.to_wav && self.audio {
            if let Err(e) = writer.write(&self.wav_header()) {
                eprintln!("Could not write to track\n{}", e);
                process::exit(4);
            };
        }

        for _ in 0..sectors {
            if let Err(e) = reader.read(&mut sector) {
                eprintln!("Could not read from {}\n{}", &a.bin_file, e);
                process::exit(4);
            }
            if self.audio && a.swap_audo_bytes {
                for i in (0..SECTOR_SIZE as usize).step_by(2) {
                    sector.swap(i, i + 1);
                }
            }
            if let Err(e) = writer.write(
                &sector[self.data_block_offset as usize
                    ..(self.data_block_offset + self.data_block_size) as usize],
            ) {
                eprintln!("Could not write to track\n{}", e);
                process::exit(4);
            };
        }
        println!("{}: {} {}MiB", self.number, filename, file_length / 1024 / 1024);
    }
}

pub enum Mode {
    Unknown,
    Audio,
    Mode1_2352,
    Mode2_2352,
    Mode2_2336,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Unknown
    }
}

impl Mode {
    const UNKNOWN: &'static str = "UNKNOWN";
    const AUDIO: &'static str = "AUDIO";
    const MODE1_2352: &'static str = "MODE1/2352";
    const MODE2_2352: &'static str = "MODE2/2352";
    const MODE2_2336: &'static str = "MODE2/2336";
}

impl AsRef<str> for Mode {
    fn as_ref(&self) -> &'static str {
        match self {
            Mode::Unknown => Mode::UNKNOWN,
            Mode::Audio => Mode::AUDIO,
            Mode::Mode1_2352 => Mode::MODE1_2352,
            Mode::Mode2_2352 => Mode::MODE2_2352,
            Mode::Mode2_2336 => Mode::MODE2_2352,
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.pad(self.as_ref())
    }
}

impl From<&str> for Mode {
    fn from(s: &str) -> Self {
        match s {
            Mode::AUDIO => Mode::Audio,
            Mode::MODE1_2352 => Mode::Mode1_2352,
            Mode::MODE2_2336 => Mode::Mode2_2336,
            Mode::MODE2_2352 => Mode::Mode2_2352,
            _ => Mode::Unknown,
        }
    }
}

enum Extension {
    Ugh,
    Iso,
    Cdr,
    Wav,
}

impl Default for Extension {
    fn default() -> Self {
        Extension::Ugh
    }
}

impl Extension {
    const UGH: &'static str = "ugh";
    const ISO: &'static str = "iso";
    const CDR: &'static str = "cdr";
    const WAV: &'static str = "wav";
}

impl AsRef<str> for Extension {
    fn as_ref(&self) -> &str {
        match self {
            Extension::Ugh => Extension::UGH,
            Extension::Iso => Extension::ISO,
            Extension::Cdr => Extension::CDR,
            Extension::Wav => Extension::WAV,
        }
    }
}

fn read_cue(args: &mut Args) -> Vec<Track> {
    let mut tracks: Vec<Track> = Vec::with_capacity(32);

    let cue = match std::fs::read_to_string(&args.cue_file) {
        Ok(f) => {
            println!("Reading CUE file:");
            f
        }
        Err(e) => {
            eprintln!("Could not open CUE file:\n{}", e);
            process::exit(2);
        }
    };

    for s in cue.lines() {
        for e in s.split_whitespace() {
            match e {
                "TRACK" => {
                    tracks.push(Default::default());
                    println!();
                    let mut t = s.split_whitespace().skip(1);
                    match t.next() {
                        Some(num_s) => match num_s.parse() {
                            Ok(num) => {
                                tracks.last_mut().unwrap().number = num;
                                print!("Track {:>2}: ", num);
                            }
                            Err(e) => {
                                eprintln!("Error parsing track number!\n{}", e);
                                process::exit(3);
                            }
                        },
                        None => process::exit(3),
                    }
                    match t.next() {
                        Some(mode) => {
                            tracks.last_mut().unwrap().mode = mode.into();
                            tracks.last_mut().unwrap().get_track_mode(args);
                            print!("{:12}", tracks.last().unwrap().mode);
                        }
                        None => process::exit(3),
                    }
                    break;
                }
                "INDEX" => {
                    let mut i = s.split_whitespace().skip(1);
                    match i.next() {
                        Some(index_s) => {
                            print!("{} ", index_s);
                        }
                        None => {
                            eprintln!("Missing index number");
                            process::exit(3);
                        }
                    }
                    match i.next() {
                        Some(time) => {
                            print!("{} ", time);
                            tracks.last_mut().unwrap().start_sector = time_to_frames(time);
                            tracks.last_mut().unwrap().start =
                                tracks.last_mut().unwrap().start_sector * SECTOR_SIZE;
                            if tracks.len() > 1 && tracks[tracks.len() - 2].stop_sector.is_none() {
                                tracks.index_mut(tracks.len() - 2).stop_sector =
                                    Some(tracks.last().unwrap().start_sector - 1);
                                tracks.index_mut(tracks.len() - 2).stop =
                                    Some(tracks.last().unwrap().start - 1);
                            }
                        }
                        None => {
                            eprintln!("Missing INDEX time");
                            process::exit(3);
                        }
                    }
                    break;
                }
                "FILE" => {
                    let mut f = s.split_whitespace().skip(1);
                    match f.next() {
                        Some(fname) => {
                            let mut filename = fname.chars();
                            filename.next();
                            filename.next_back();
                            if args.bin_file.is_empty() {
                                args.bin_file = String::from(filename.as_str());
                                eprintln!("BIN file not supplied. Reading BIN file from CUE file");
                            } else if filename.as_str() != args.bin_file.split('/').last().unwrap()
                            {
                                eprintln!("Filename in CUE file doesn't match filename provided")
                            }
                        }
                        None => eprintln!("Error reading FILE row"),
                    }
                    break;
                }
                _ => continue,
            }
        }
    }
    if tracks.is_empty() {
        eprintln!("No valid CUE data found");
        process::exit(3);
    }
    // Get last track stopsector form the size of the file
    let bin_file_size = match fs::metadata(&args.bin_file) {
        Ok(metadata) => metadata.len(),
        Err(e) => {
            eprintln!("Could not open BIN file\n{}", e);
            process::exit(2);
        }
    };
    tracks.last_mut().unwrap().stop = Some(bin_file_size - 1);
    tracks.last_mut().unwrap().stop_sector =
        Some(tracks.last().unwrap().stop.unwrap() / SECTOR_SIZE);
    println!("\n");
    tracks
}

fn time_to_frames(s: &str) -> u64 {
    let mut duration = [0u64; 3]; // minutes,seconds,frames

    for (c, t) in s.split(':').zip(duration.iter_mut()) {
        *t = match c.parse() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{}:", e);
                process::exit(3)
            }
        };
    }
    75 * (duration[0] * 60 + duration[1]) + duration[2]
}

fn print_help() {
    println!(
        "Usage: bchunk [-r] [-p (PSX)] [-w (wav)] [-s (swabaudio)]
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

fn main() {
    println!("rbchunk v1.0.0
https://gitlab.com/TheMaxus/rbchunk
Based on bchunk by Heikki Hannikainen <hessu@hes.iki.fi>\n");
    let mut args = Args::new();

    let tracks = read_cue(&mut args);

    // Opening file in main so that reader has a liftime of the main function
    // This way we save around 700Kb of memory allocations
    let in_file = match fs::File::open(&args.bin_file) {
        Ok(i_file) => i_file,
        Err(e) => {
            eprintln!("Could not open BIN {}", e);
            process::exit(2);
        }
    };
    let mut reader: std::io::BufReader<&std::fs::File> =
        std::io::BufReader::with_capacity(SECTOR_SIZE as usize * 16, &in_file);

    println!("Writing tracks:\n");
    for t in &tracks {
        t.write_to_file(&mut reader, &args);
    }
}
