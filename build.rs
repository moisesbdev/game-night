// Bundles the page (ui/) into the program, so Game Night is one file with nothing to install beside it.
use std::fmt::Write;
use std::path::Path;

fn main() {
    let ui = Path::new("ui");
    println!("cargo:rerun-if-changed=ui");
    let mut files = vec![];
    for top in ["index.html", "style.css", "favicon.svg"] {
        files.push(top.to_string());
    }
    for (dir, ext) in [("js", "js"), ("fonts", "ttf")] {
        if let Ok(rd) = std::fs::read_dir(ui.join(dir)) {
            let mut v: Vec<String> = rd
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(&format!(".{ext}")))
                .map(|n| format!("{dir}/{n}"))
                .collect();
            v.sort();
            files.extend(v);
        }
    }
    let mut out = String::from("pub static ASSETS: &[(&str, &[u8])] = &[\n");
    for f in files {
        let p = std::fs::canonicalize(ui.join(&f)).unwrap_or_else(|_| panic!("missing ui/{f}"));
        writeln!(out, "    ({f:?}, include_bytes!({:?})),", p.to_string_lossy()).unwrap();
    }
    out.push_str("];\n");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("assets.rs"), out).unwrap();
    #[cfg(windows)]
    {
        // the window and taskbar icon come from the page's favicon at run time; nothing else to embed here
    }
}
