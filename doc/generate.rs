#!/usr/bin/env -S cargo +nightly -Zscript

fn main() {
    let dir = std::fs::read_dir("doc/dot").unwrap();

    for item in dir {
        let item = item.unwrap();
        let path = item.path();
        let extless_out_path: &std::path::Path = path.file_stem().unwrap().as_ref();
        let out_path = std::path::Path::new("doc/svg").join(extless_out_path.with_extension("svg"));
        let out = std::process::Command::new("dot")
            .args([
                "-Tsvg_inline",
                "-Gsvg:comments=false",
                item.path().display().to_string().as_str(),
            ])
            .output()
            .unwrap();
        let s = std::str::from_utf8(&out.stdout).unwrap();
        eprintln!("{}", std::str::from_utf8(&out.stderr).unwrap());
        if !out.status.success() {
            eprintln!("[ERR] dot failed");
            return;
        }
        let stripped = s.replace('\n', "");
        std::fs::write(out_path, stripped).unwrap();
    }
    eprintln!("[OK] Regenerated svg graphs");
}
