//! Command line pxtone player
#![forbid(unsafe_code)]

use {
    clap::Parser,
    crossterm::{QueueableCommand, SynchronizedUpdate, cursor, terminal},
    ptcow::{Herd, MooInstructions, MooPlan, SampleRate, Unit, VoiceData, moo_prepare},
    std::{
        io::{ErrorKind, IsTerminal, Write as _},
        iter::zip,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
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

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let vis = !args.no_vis;
    let mut stderr = std::io::stderr().lock();
    if vis {
        writeln!(
            stderr,
            "File: {}\nRate: {}\nBufsize: {}",
            args.path.display(),
            args.sample_rate,
            args.buf_size
        )?;
    }
    let data = match std::fs::read(&args.path) {
        Ok(data) => data,
        Err(e) => {
            writeln!(stderr, "Failed to read '{}': {e}", args.path.display())?;
            return Err(std::io::Error::other("File read error"));
        }
    };
    let (song, mut herd, mut ins) = match ptcow::read_song(&data, args.sample_rate) {
        Ok((song, herd, ins)) => (song, herd, ins),
        Err(e) => {
            writeln!(
                stderr,
                "Failed to read '{}' as PxTone: {e}",
                args.path.display()
            )?;
            return Err(std::io::Error::other("PxTone read error"));
        }
    };
    let plan = MooPlan {
        start_pos: ptcow::StartPosPlan::Sample(0),
        meas_end: None,
        meas_repeat: None,
        loop_: !args.no_loop,
    };
    moo_prepare(&mut ins, &mut herd, &song, &plan);

    let mut buf = vec![0i16; args.buf_size];
    let mut writer = std::io::stdout().lock();
    if writer.is_terminal() {
        writeln!(
            stderr,
            "You don't want to write sample data to a terminal. Trust me."
        )?;
        return Err(std::io::Error::other(
            "Attempting to write sample data to terminal",
        ));
    }
    let stop = Arc::new(AtomicBool::new(false));
    if vis {
        stderr.queue(terminal::EnterAlternateScreen)?;
        stderr.queue(terminal::DisableLineWrap)?;
        stderr.queue(cursor::Hide)?;
        let stop = stop.clone();
        ctrlc::set_handler(move || {
            stop.store(true, Ordering::Relaxed);
        })
        .unwrap();
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
        if stop.load(Ordering::Relaxed) {
            writeln!(stderr, "Gotta stop!")?;
            break;
        }
        if vis {
            stderr.sync_update(|stderr| print(stderr, &song, &herd, &ins))??;
        }
    }
    stderr.queue(terminal::LeaveAlternateScreen)?;
    stderr.queue(cursor::Show)?;
    stderr.flush()?;
    Ok(())
}

fn print(
    stderr: &mut std::io::StderrLock,
    song: &ptcow::Song,
    herd: &Herd,
    ins: &MooInstructions,
) -> std::io::Result<()> {
    let ratio = f64::from(herd.smp_count) / f64::from(herd.smp_end);
    stderr.queue(terminal::Clear(terminal::ClearType::All))?;
    if !song.text.name.is_empty() {
        writeln!(stderr, "= {} =", song.text.name)?;
    }
    if !song.text.comment.is_empty() {
        writeln!(stderr, "\n{}\n", song.text.comment)?;
    }
    writeln!(
        stderr,
        "{}/{} ({:.02}%)",
        herd.smp_count,
        herd.smp_end,
        ratio * 100.,
    )?;
    let (name_widths, name_max) = name_widths(&herd.units);
    for (unit, nw) in zip(herd.units.iter(), name_widths) {
        let val: i32 = unit.pan_time_bufs.iter().flatten().sum();
        let name: &str = &unit.name;
        let voice = &ins.voices[unit.voice_idx];
        for (i, slot) in voice.slots().enumerate() {
            let kind = match &slot.data {
                VoiceData::Noise(_) => "🥁",
                VoiceData::Pcm(_) => "🎤",
                VoiceData::Wave(_) => "〰️",
                VoiceData::OggV(_) => "🐠", // Ogg/Vorbis logo is a fish
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
            writeln!(stderr, "{cow}{name}{fill} {kind} {moo}")?;
        }
    }
    stderr.queue(cursor::MoveTo(0, 0))?;
    Ok(())
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
