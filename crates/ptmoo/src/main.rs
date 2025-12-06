//! Command line pxtone player
#![forbid(unsafe_code)]

use {
    clap::Parser,
    ptcow::{Herd, MooInstructions, MooPlan, SampleRate, Unit, VoiceData, moo_prepare},
    std::{
        io::{ErrorKind, IsTerminal, Write as _},
        iter::zip,
        path::PathBuf,
    },
    string_width::DisplayWidth,
};

#[derive(clap::Parser)]
struct Args {
    /// Path to song
    path: PathBuf,
    /// Output sample rate
    #[arg(short = 'r', long, default_value = "44100")]
    sample_rate: SampleRate,
    /// Buffer size in bytes to render to
    #[arg(short = 'b', long, default_value = "16384")]
    buf_size: usize,
    /// Don't loop the song
    #[arg(long)]
    no_loop: bool,
    /// Disable visualization/info dump
    #[arg(long)]
    no_vis: bool,
}

fn main() {
    let args = Args::parse();
    let vis = !args.no_vis;
    if vis {
        eprintln!(
            "File: {}\nRate: {}\nBufsize: {}",
            args.path.display(),
            args.sample_rate,
            args.buf_size
        );
    }
    let data = match std::fs::read(&args.path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to read '{}': {e}", args.path.display());
            return;
        }
    };
    let (song, mut herd, mut ins) = ptcow::read_song(&data, args.sample_rate).unwrap();
    if vis {
        eprintln!("\x1b[?25l");
        ctrlc::set_handler(move || {
            eprintln!("\x1bc");
            eprintln!("\x1b[?25h");
        })
        .unwrap();
    }

    let plan = MooPlan {
        start_pos: ptcow::StartPosPlan::Sample(0),
        meas_end: 0,
        meas_repeat: 0,
        loop_: !args.no_loop,
    };
    moo_prepare(&mut ins, &mut herd, &song, &plan);

    let mut buf = vec![0i16; args.buf_size];
    let mut writer = std::io::stdout().lock();
    if writer.is_terminal() {
        eprintln!("You don't want to write sample data to a terminal. Trust me.");
        return;
    }
    if vis {
        eprintln!("Playing {}", song.text.name);
        eprintln!("Comment:\n{}", song.text.comment);
    }

    while herd.moo(&ins, &song, &mut buf, true) {
        let result = writer.write_all(bytemuck::cast_slice(&buf));
        if let Err(e) = result {
            match e.kind() {
                ErrorKind::BrokenPipe => {
                    break;
                }
                _ => panic!("I/O error: {e}"),
            }
        }
        if vis {
            print(&herd, &ins);
        }
    }
    if vis {
        eprintln!("\x1bc");
        eprintln!("\x1b[?25h");
    }
}

fn print(herd: &Herd, ins: &MooInstructions) {
    let ratio = f64::from(herd.smp_count) / f64::from(herd.smp_end);
    eprintln!(
        "\x1b[K{}/{} ({:.02}%)",
        herd.smp_count,
        herd.smp_end,
        ratio * 100.
    );
    let mut total_shown = 0;
    let (name_widths, name_max) = name_widths(&herd.units);
    for (unit, nw) in zip(&herd.units, name_widths) {
        let val: i32 = unit.pan_time_bufs.iter().flatten().sum();
        let name: &str = &unit.name;
        let voice = &ins.voices[unit.voice_idx];
        for (i, unit) in voice.units.iter().enumerate() {
            let kind = match &unit.data {
                VoiceData::Noise(_) => "🥁",
                VoiceData::Pcm(_) => "🎤",
                VoiceData::Wave(_) => "〰️",
            };
            let ratio = f64::from(val.abs()) / 4_194_304.0;
            #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let n_moo = (ratio * 64.).ceil() as usize;
            let fill = " ".repeat(name_max - nw);
            let moo = "🐄".repeat(n_moo);
            let (name, cow) = if i == 0 {
                (name, "🐮")
            } else {
                (&*" ".repeat(nw), "╰─")
            };
            eprintln!("\x1b[K{cow}{name}{fill} {kind} {moo}");
            total_shown += 1;
        }
    }
    let up = total_shown + 1;
    eprint!("\x1b[{up}A\r");
}

fn name_widths(units: &[Unit]) -> (Vec<usize>, usize) {
    let mut out = Vec::new();
    let mut max = 0;
    for unit in units {
        let dw = unit.name.display_width();
        out.push(dw);
        max = std::cmp::max(max, dw);
    }
    (out, max)
}
