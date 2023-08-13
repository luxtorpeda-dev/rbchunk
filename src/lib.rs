use std::fmt::Display;
use std::fs;
use std::io;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::io::{Error, ErrorKind};
use std::mem::swap;
use std::ops::IndexMut;

const WAV_RIFF_HEADER_LENGTH: u32 = 12;
const WAV_FORMAT_HEADER_LENGTH: u32 = 24;
const WAV_DATA_HEADER_LENGTH: u32 = 8;
const WAV_HEADER_LENGTH: u32 =
    WAV_RIFF_HEADER_LENGTH + WAV_FORMAT_HEADER_LENGTH + WAV_DATA_HEADER_LENGTH;

const SECTOR_SIZE: u64 = 2352;

#[derive(Default)]
pub struct Args {
    pub output_name: String,
    pub bin_file: String,
    pub cue_file: String,
    pub verbose: bool,
    pub psx_truncate: bool,
    pub raw: bool,
    pub swap_audo_bytes: bool,
    pub to_wav: bool,
}

impl Args {
    pub fn new(mut options: Args) -> Self {
        /*
         * If binfile is not supplied we can read it from CUE file
         * This could have been done in a better way, but for the sake of
         * compatibility with the original program we have to do it this way
         */
        if options.cue_file.is_empty() {
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
        let reallen =
            (self.stop_sector.unwrap() - self.start_sector + 1) * self.data_block_size as u64;

        let wav_header = [
            // RIFF header
            "RIFF".as_bytes(),
            (reallen as u32 + WAV_DATA_HEADER_LENGTH + WAV_FORMAT_HEADER_LENGTH + 4)
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

    fn write_to_file(&self, reader: &mut BufReader<&std::fs::File>, a: &Args) -> io::Result<()> {
        let filename = format!(
            "{}{:0>2}.{}",
            a.output_name,
            self.number,
            self.extension.as_ref()
        );
        let sectors = self.stop_sector.unwrap() - self.start_sector + 1;
        let mut file_length = sectors * self.data_block_size as u64;
        let mut sector = [0u8; SECTOR_SIZE as usize];

        let out_file = match fs::File::create(&filename) {
            Ok(t_file) => t_file,
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Could not write to track: {}", e),
                ))
            }
        };

        let mut writer: std::io::BufWriter<&std::fs::File> =
            std::io::BufWriter::with_capacity(SECTOR_SIZE as usize * 16, &out_file);

        if let Err(e) = reader.seek(SeekFrom::Start(self.start)) {
            return Err(Error::new(
                ErrorKind::Other,
                format!("Could not seek to track location {}", e),
            ));
        }

        if a.to_wav && self.audio {
            file_length += WAV_HEADER_LENGTH as u64;
            if let Err(e) = writer.write(&self.wav_header()) {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Could not write to track {}", e),
                ));
            };
        }

        for _ in 0..sectors {
            if let Err(e) = reader.read(&mut sector) {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Could not read from {} {}", &a.bin_file, e),
                ));
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
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Could not write to track {}", e),
                ));
            };
        }

        if a.verbose {
            println!(
                "{}: {} {}MiB",
                self.number,
                filename,
                file_length / 1024 / 1024
            );
        }

        Ok(())
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

fn read_cue(args: &mut Args) -> io::Result<Vec<Track>> {
    let mut tracks: Vec<Track> = Vec::with_capacity(32);

    let cue = match std::fs::read_to_string(&args.cue_file) {
        Ok(f) => f,
        Err(e) => {
            return Err(Error::new(
                ErrorKind::Other,
                format!("Could not open CUE file: {}", e),
            ))
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
                                if args.verbose {
                                    print!("Track {:>2}: ", num);
                                }
                            }
                            Err(e) => {
                                return Err(Error::new(
                                    ErrorKind::Other,
                                    format!("Error parsing track number! {}", e),
                                ))
                            }
                        },
                        None => return Err(Error::new(ErrorKind::Other, "Unknown error")),
                    }
                    match t.next() {
                        Some(mode) => {
                            tracks.last_mut().unwrap().mode = mode.into();
                            tracks.last_mut().unwrap().get_track_mode(args);
                            if args.verbose {
                                print!("{:12}", tracks.last().unwrap().mode);
                            }
                        }
                        None => return Err(Error::new(ErrorKind::Other, "Unknown error")),
                    }
                    break;
                }
                "INDEX" => {
                    let mut i = s.split_whitespace().skip(1);
                    match i.next() {
                        Some(index_s) => {
                            if args.verbose {
                                print!("{} ", index_s);
                            }
                        }
                        None => return Err(Error::new(ErrorKind::Other, "Missing index number")),
                    }
                    match i.next() {
                        Some(time) => {
                            if args.verbose {
                                print!("{} ", time);
                            }
                            tracks.last_mut().unwrap().start_sector = time_to_frames(time).unwrap();
                            tracks.last_mut().unwrap().start =
                                tracks.last_mut().unwrap().start_sector * SECTOR_SIZE;
                            if tracks.len() > 1 && tracks[tracks.len() - 2].stop_sector.is_none() {
                                tracks.index_mut(tracks.len() - 2).stop_sector =
                                    Some(tracks.last().unwrap().start_sector - 1);
                                tracks.index_mut(tracks.len() - 2).stop =
                                    Some(tracks.last().unwrap().start - 1);
                            }
                        }
                        None => return Err(Error::new(ErrorKind::Other, "Missing INDEX time")),
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
                                if args.verbose {
                                    eprintln!(
                                        "BIN file not supplied. Reading BIN file from CUE file"
                                    );
                                }
                            } else if filename.as_str() != args.bin_file.split('/').last().unwrap()
                            {
                                if args.verbose {
                                    eprintln!(
                                        "Filename in CUE file doesn't match filename provided"
                                    )
                                }
                            }
                        }
                        None => return Err(Error::new(ErrorKind::Other, "Error reading FILE row")),
                    }
                    break;
                }
                _ => continue,
            }
        }
    }
    if tracks.is_empty() {
        return Err(Error::new(ErrorKind::Other, "No valid CUE data found"));
    }
    // Get last track stopsector form the size of the file
    let bin_file_size = match fs::metadata(&args.bin_file) {
        Ok(metadata) => metadata.len(),
        Err(e) => {
            return Err(Error::new(
                ErrorKind::Other,
                format!("Could not open BIN file\n{}", e),
            ))
        }
    };
    tracks.last_mut().unwrap().stop = Some(bin_file_size - 1);
    tracks.last_mut().unwrap().stop_sector =
        Some(tracks.last().unwrap().stop.unwrap() / SECTOR_SIZE);

    Ok(tracks)
}

fn time_to_frames(s: &str) -> io::Result<u64> {
    let mut duration = [0u64; 3]; // minutes,seconds,frames

    for (c, t) in s.split(':').zip(duration.iter_mut()) {
        *t = match c.parse() {
            Ok(t) => t,
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("parse int error on time_to_frames {}", e),
                ))
            }
        };
    }
    Ok(75 * (duration[0] * 60 + duration[1]) + duration[2])
}

pub fn convert(options: Args) -> io::Result<()> {
    let mut args = Args::new(options);

    let tracks = match read_cue(&mut args) {
        Ok(i_tracks) => i_tracks,
        Err(e) => return Err(e),
    };

    // Opening file in convert so that reader has a liftime of the convert function
    // This way we save around 700Kb of memory allocations
    let in_file = match fs::File::open(&args.bin_file) {
        Ok(i_file) => i_file,
        Err(e) => return Err(e),
    };
    let mut reader: std::io::BufReader<&std::fs::File> =
        std::io::BufReader::with_capacity(SECTOR_SIZE as usize * 16, &in_file);

    for t in &tracks {
        match t.write_to_file(&mut reader, &args) {
            Ok(()) => {}
            Err(err) => return Err(err),
        }
    }

    Ok(())
}
