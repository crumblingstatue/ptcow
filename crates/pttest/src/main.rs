//! Testing utilities for ptcow

use {
    anstyle::AnsiColor,
    clap::Parser,
    ptcow::NoiseTable,
    std::{
        error::Error,
        io::{self},
        path::PathBuf,
    },
};

#[derive(clap::Parser)]
enum Args {
    DumpNoiseTables { out_path: PathBuf },
    CompareNoiseTables,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if let Err(e) = std::fs::create_dir_all(basedir()) {
        eprintln!("Error: Failed to create test dir: {e}");
    }
    match args {
        Args::DumpNoiseTables { out_path } => dump_noise_tables_file(out_path)?,
        Args::CompareNoiseTables => cmp_noise_tables()?,
    }
    Ok(())
}

fn dump_noise_tables_buf() -> Vec<u8> {
    let table = NoiseTable::generate();
    let tbls = table.inner();
    let mut out = Vec::new();
    for tbl in tbls {
        out.extend_from_slice(bytemuck::cast_slice(tbl));
    }
    out
}

fn dump_noise_tables_file(out_path: PathBuf) -> io::Result<()> {
    std::fs::write(out_path, dump_noise_tables_buf())?;
    Ok(())
}

fn basedir() -> PathBuf {
    std::env::temp_dir().join("ptcow-test")
}

fn cmp_noise_tables() -> Result<(), Box<dyn Error>> {
    let path = basedir().join("clean-wavetable.pcm");
    if !path.exists() {
        return Err(format!("Need clean file at '{}'", path.display()).into());
    }
    let clean = std::fs::read(path)?;
    let dirty = dump_noise_tables_buf();
    if clean == dirty {
        pass("Noise tables match");
    } else {
        fail("Noise table mismatch");
    }
    Ok(())
}

fn pass(msg: &str) {
    let style = anstyle::Style::new()
        .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green)))
        .bold();
    eprintln!("{style}[PASS]{style:#} {msg}");
}

fn fail(msg: &str) {
    let style = anstyle::Style::new()
        .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)))
        .bold();
    eprintln!("{style}[FAIL]{style:#} {msg}");
}
