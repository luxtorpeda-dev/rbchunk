# BinChunker in Rust

As the title says this is basically **bchunk** written
in Rust, it has all the same features as the original
and some improvements. Mainly in some cases it's easier
to use and it's slightly faster (noticable only on RAM
disk or fast SSD as storage still remains the biggest bottleneck).

This adds support for rbchunk to be a library that can be imported by other projects, based on `https://gitlab.com/TheMaxus/rbchunk.git`. An example cli tool can be seen in `examples/cli.rs`

## How to use

### Basic usage (after building example):

```
rbchunk [-w] [-s] foo.cue
```

If only one file is supplied the program will treat it as a CUE file.

This will extract tracks from the .bin file specified in CUE
sheet to the current directory with names like foo01.cdr. `-w`
switch will extract files in .wav format and `-s` flag will
switch byte order (use this if you get white noise or
otherwise corrupted audio in the output files).


```
rbchunk [-ws] foo.bin foo.cue [something]
```

This will do the same as above but you can specify the BIN file and output name.

If two or three files are supplied first will always be treated as BIN file, second as CUE file and third as a filename for the output. Any other arguments will be ignored.

### Basic usage as library:

```
let args = read_args(); // Providing your own rbchunk::Args here
match rbchunk::convert(args) {
    Ok(()) => println!("Conversion complete!"),
    Err(err) => {
        println!("Error on conversion: {}", err);
        process::exit(1);
    }
}
```

## Contribution

Feel free to contribute to the project, but try to avoid any external dependencies, as I try to keep this program rather small.

## Compillation

 - `git clone https://github.com/luxtorpeda-dev/rbchunk`
 - `cargo build -r`

## Releasing a New Version

* First make sure all pull requests have been merged that are wanted for the release. Then ```git pull``` on the master branch.
* Make sure cargo-release is installed locally ```cargo install cargo-release``` (https://github.com/crate-ci/cargo-release)
* Run ```cargo release version major``` to do a dry run of the version update. Can also use minor, patch, etc
* Run again if dry run successful: ```cargo release version major --execute```. Make sure to commit & push to git if needed.
* Run ```cargo release``` to do a dry run of the release.
* Run ```cargo release --execute``` if dry run successful. This step will push the git tags and a github action will run to publish the new version to crate.

## Credits

This program is mostly based on bchunk by Heikki Hannikainen <hessu@hes.iki.fi>,  
which in turn is based on BinChunker by Bob Marietta <marietrg@SLU.EDU>

Other contributors to bchunk:
 - Colas Nahaboo <Colas@Nahaboo.com>, 1999
 - Godmar Back <gback@cs.utah.edu>, 2001
 - Matthew Green <mrg@eterna.com.au>, 2003
 - Piotr Kaczuba <pepe@attika.ath.cx>, 2009
 - Reuben Thomas <rrt@femur.dyndns.org>, 2008
 - Yegor Timoshenko <yegortimoshenko@gmail.com>, 2017
