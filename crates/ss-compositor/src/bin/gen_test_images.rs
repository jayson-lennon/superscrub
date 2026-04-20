//! Generate test images for manual compositor inspection.
//!
//! Creates simple solid-color and gradient PNGs in the target directory.

use image::{Rgba, RgbaImage};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples/test-frame".to_string());

    std::fs::create_dir_all(&dir).expect("create dir");

    // Background: dark blue vertical gradient (640x480).
    let mut bg = RgbaImage::new(640, 480);
    for y in 0..480 {
        let t = y as f32 / 480.0;
        let r = (20.0 + t * 15.0) as u8;
        let g = (24.0 + t * 10.0) as u8;
        let b = (48.0 + t * 30.0) as u8;
        for x in 0..640 {
            bg.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
    bg.save(format!("{dir}/bg.png")).expect("save bg");
    eprintln!("  wrote {dir}/bg.png (640x480 gradient)");

    // Overlay: green rounded rectangle (300x200).
    let mut overlay = RgbaImage::new(300, 200);
    let edge = 12.0;
    let corner_r = 12.0;
    for y in 0..200 {
        for x in 0..300 {
            let xf = x as f32;
            let yf = y as f32;

            // Check if inside rounded rectangle.
            let in_body = xf >= edge && xf < 300.0 - edge && yf >= edge && yf < 200.0 - edge;

            // Check corners.
            let dx = if xf < 300.0 / 2.0 {
                xf - edge
            } else {
                xf - (300.0 - edge)
            };
            let dy = if yf < 200.0 / 2.0 {
                yf - edge
            } else {
                yf - (200.0 - edge)
            };
            let in_corner = !(xf < edge || xf >= 300.0 - edge || yf < edge || yf >= 200.0 - edge)
                || (dx * dx + dy * dy) <= corner_r * corner_r;

            if in_body || in_corner {
                let alpha = if !(3.0..297.0).contains(&xf) || !(3.0..197.0).contains(&yf) {
                    180
                } else {
                    220
                };
                overlay.put_pixel(x, y, Rgba([40, 200, 80, alpha]));
            }
        }
    }
    overlay
        .save(format!("{dir}/overlay.png"))
        .expect("save overlay");
    eprintln!("  wrote {dir}/overlay.png (300x200 green rounded rect)");

    // Accent: solid red circle (160x160).
    let mut accent = RgbaImage::new(160, 160);
    let cx = 80.0_f32;
    let cy = 80.0_f32;
    let radius = 70.0_f32;
    for y in 0..160 {
        for x in 0..160 {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= radius * radius {
                let dist = dist_sq.sqrt();
                let alpha = if dist > radius - 3.0 { 200 } else { 255 };
                accent.put_pixel(x, y, Rgba([220, 50, 50, alpha]));
            }
        }
    }
    accent
        .save(format!("{dir}/accent.png"))
        .expect("save accent");
    eprintln!("  wrote {dir}/accent.png (160x160 red circle)");

    // Title bar: semi-transparent white strip (500x40).
    let mut strip = RgbaImage::new(500, 40);
    for y in 0..40 {
        for x in 0..500 {
            let alpha = if !(2..38).contains(&y) { 160 } else { 230 };
            strip.put_pixel(x, y, Rgba([240, 240, 240, alpha]));
        }
    }
    strip.save(format!("{dir}/title.png")).expect("save title");
    eprintln!("  wrote {dir}/title.png (500x40 white strip)");

    eprintln!("Done. Generated 4 test images in {dir}/");
}
