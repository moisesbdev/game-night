// Bundles the page (ui/) into the program, so Game Night is one file with nothing to install beside it.
// On Windows it also gives the program its icon (drawn here, no image files), version details and a manifest.
use std::fmt::Write;
use std::path::Path;

fn main() {
    let ui = Path::new("ui");
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=build.rs");
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
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(Path::new(&out_dir).join("assets.rs"), out).unwrap();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let ico = Path::new(&out_dir).join("gamenight.ico");
        std::fs::write(&ico, icon::ico(&[16, 24, 32, 48, 64, 128, 256])).unwrap();
        let manifest = Path::new(&out_dir).join("gamenight.manifest");
        std::fs::write(&manifest, MANIFEST).unwrap();
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico.to_str().unwrap())
            .set_manifest_file(manifest.to_str().unwrap())
            .set("ProductName", "Game Night")
            .set("FileDescription", "Game Night: pick what to play tonight")
            .set("CompanyName", "The Game Night contributors")
            .set("LegalCopyright", "MIT licence, The Game Night contributors")
            .set("OriginalFilename", "gamenight.exe");
        if let Err(e) = res.compile() {
            println!("cargo:warning=no icon or version details embedded: {e}");
        }
    }
}

/// Sharp on high-DPI screens, modern common controls (the folder picker), UTF-8 everywhere, Windows 10 and 11.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="GameNight" version="1.0.0.0"/>
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>
    <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
    <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
  </windowsSettings></application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application>
    <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
  </application></compatibility>
</assembly>
"#;

/// The app icon, drawn like the page's favicon: a sea-glass tile with the smiling moon, anti-aliased, as a Windows .ico.
mod icon {
    type Rgba = [f32; 4];
    const TILE: Rgba = [0.918, 0.969, 0.961, 1.0];
    const MOON: Rgba = [0.624, 0.890, 0.863, 1.0];
    const SPOT: Rgba = [0.498, 0.827, 0.796, 1.0];
    const INK: Rgba = [0.071, 0.204, 0.231, 1.0];
    const STAR: Rgba = [0.949, 0.769, 0.420, 1.0];

    /// Distance from (px, py) to a quadratic curve, by sampling it (the face's eyes and smile).
    fn quad_dist(px: f32, py: f32, a: (f32, f32), c: (f32, f32), b: (f32, f32)) -> f32 {
        (0..=24)
            .map(|i| {
                let t = i as f32 / 24.0;
                let u = 1.0 - t;
                let x = u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0;
                let y = u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1;
                ((px - x).powi(2) + (py - y).powi(2)).sqrt()
            })
            .fold(f32::MAX, f32::min)
    }

    /// The colour at a point of the 64x64 design.
    fn sample(x: f32, y: f32) -> Rgba {
        let over = |under: Rgba, top: Rgba| -> Rgba {
            let a = top[3];
            [top[0] * a + under[0] * (1.0 - a), top[1] * a + under[1] * (1.0 - a), top[2] * a + under[2] * (1.0 - a), a + under[3] * (1.0 - a)]
        };
        let in_round_rect = {
            let (dx, dy) = ((x - 32.0).abs() - 16.0, (y - 32.0).abs() - 16.0);
            dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) <= 16.0
        };
        let mut c: Rgba = if in_round_rect { TILE } else { [0.0; 4] };
        let d = (x - 32.0).hypot(y - 35.0);
        if d <= 23.25 {
            c = over(c, INK);
        }
        if d <= 20.75 {
            c = over(c, MOON);
            if (x - 45.0).hypot(y - 26.0) <= 3.4 || (x - 19.0).hypot(y - 48.0) <= 2.8 {
                c = over(c, SPOT);
            }
            for (a, m, b) in
                [((22.0, 34.0), (25.5, 38.0), (29.0, 34.0)), ((35.0, 34.0), (38.5, 38.0), (42.0, 34.0)), ((28.0, 43.0), (32.0, 47.0), (36.0, 43.0))]
            {
                if quad_dist(x, y, a, m, b) <= 1.25 {
                    c = over(c, INK);
                }
            }
        }
        let (sx, sy) = (x - 52.0, y - 13.0);
        if sx.abs() + sy.abs() <= 5.0 && (sx.abs() <= 1.6 || sy.abs() <= 1.6 || sx.abs() + sy.abs() <= 2.6) {
            c = over(c, STAR);
        }
        c
    }

    /// One size as bottom-up BGRA rows, each pixel averaged over a 4x4 grid of samples.
    fn pixels(n: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity((n * n * 4) as usize);
        for row in (0..n).rev() {
            for col in 0..n {
                let mut acc = [0.0f32; 4];
                for sy in 0..4 {
                    for sx in 0..4 {
                        let s = sample((col as f32 + (sx as f32 + 0.5) / 4.0) * 64.0 / n as f32, (row as f32 + (sy as f32 + 0.5) / 4.0) * 64.0 / n as f32);
                        for k in 0..3 {
                            acc[k] += s[k] * s[3];
                        }
                        acc[3] += s[3];
                    }
                }
                let a = acc[3] / 16.0;
                let col_of = |k: usize| if acc[3] > 0.0 { (acc[k] / acc[3] * 255.0).round() as u8 } else { 0 };
                out.extend_from_slice(&[col_of(2), col_of(1), col_of(0), (a * 255.0).round() as u8]);
            }
        }
        out
    }

    /// A Windows icon file holding each size as a 32-bit image with alpha.
    pub fn ico(sizes: &[u32]) -> Vec<u8> {
        let images: Vec<Vec<u8>> = sizes
            .iter()
            .map(|&n| {
                let mask_row = n.div_ceil(32) * 4;
                let mut b = vec![];
                for v in [40u32, n, n * 2] {
                    b.extend_from_slice(&v.to_le_bytes());
                }
                b.extend_from_slice(&1u16.to_le_bytes());
                b.extend_from_slice(&32u16.to_le_bytes());
                for v in [0u32, n * n * 4 + mask_row * n, 0, 0, 0, 0] {
                    b.extend_from_slice(&v.to_le_bytes());
                }
                b.extend(pixels(n));
                b.extend(std::iter::repeat_n(0u8, (mask_row * n) as usize));
                b
            })
            .collect();
        let mut out = vec![0, 0, 1, 0];
        out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
        let mut offset = 6 + 16 * sizes.len() as u32;
        for (n, img) in sizes.iter().zip(&images) {
            let dim = if *n >= 256 { 0 } else { *n as u8 };
            out.extend_from_slice(&[dim, dim, 0, 0]);
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&32u16.to_le_bytes());
            out.extend_from_slice(&(img.len() as u32).to_le_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            offset += img.len() as u32;
        }
        for img in images {
            out.extend(img);
        }
        out
    }
}
