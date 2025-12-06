//! Test binary for serialization

use std::process::ExitCode;

fn main() -> ExitCode {
    let in_file = std::env::args().nth(1).expect("Need .ptcop file as arg");
    let in_bytes = std::fs::read(in_file).unwrap();
    let (song, herd, ins) = ptcow::read_song(&in_bytes, 44_100).unwrap();
    let out_bytes = ptcow::serialize_project(&song, &herd, &ins).unwrap();
    if in_bytes != out_bytes {
        eprintln!("Mismatch.");
        std::fs::write("/tmp/in.ptcop", &in_bytes).unwrap();
        std::fs::write("/tmp/out.ptcop", &out_bytes).unwrap();
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
