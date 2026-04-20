use image::{Rgba, RgbaImage};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples/demo".to_string());

    std::fs::create_dir_all(&dir).expect("create dir");

    // Background: deep teal gradient (1920x1080).
    let mut bg = RgbaImage::new(1920, 1080);
    for y in 0..1080 {
        let t = y as f32 / 1080.0;
        let r = (10.0 + t * 20.0) as u8;
        let g = (40.0 + t * 40.0) as u8;
        let b = (60.0 + t * 50.0) as u8;
        for x in 0..1920 {
            bg.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
    bg.save(format!("{dir}/bg.png")).expect("save bg");
    eprintln!("  wrote {dir}/bg.png (1920x1080 teal gradient)");

    // Card: frosted glass rounded rectangle (800x500).
    let mut card = RgbaImage::new(800, 500);
    let margin = 8.0;
    let cr = 20.0;
    for y in 0..500 {
        for x in 0..800 {
            if inside_rounded_rect(x as f32, y as f32, 800.0, 500.0, margin, cr) {
                let edge = !(12..788).contains(&x) || !(12..488).contains(&y);
                let alpha = if edge { 160 } else { 210 };
                card.put_pixel(x, y, Rgba([200, 210, 220, alpha]));
            }
        }
    }
    card.save(format!("{dir}/card.png")).expect("save card");
    eprintln!("  wrote {dir}/card.png (800x500 frosted card)");

    // Badge: bright coral circle (300x300).
    let mut badge = RgbaImage::new(300, 300);
    let cx = 150.0_f32;
    let cy = 150.0_f32;
    let radius = 130.0_f32;
    for y in 0..300 {
        for x in 0..300 {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= radius * radius {
                let dist = dist_sq.sqrt();
                let edge = dist > radius - 4.0;
                let alpha = if edge { 200 } else { 255 };
                badge.put_pixel(x, y, Rgba([240, 90, 70, alpha]));
            }
        }
    }
    badge.save(format!("{dir}/badge.png")).expect("save badge");
    eprintln!("  wrote {dir}/badge.png (300x300 coral circle)");

    // Stripe: semi-transparent white accent bar (1200x60).
    let mut stripe = RgbaImage::new(1200, 60);
    for y in 0..60 {
        for x in 0..1200 {
            let alpha = if !(4..56).contains(&y) { 140 } else { 220 };
            stripe.put_pixel(x, y, Rgba([255, 255, 255, alpha]));
        }
    }
    stripe
        .save(format!("{dir}/stripe.png"))
        .expect("save stripe");
    eprintln!("  wrote {dir}/stripe.png (1200x60 white bar)");

    // Dot: small golden circle (120x120) for decoration.
    let mut dot = RgbaImage::new(120, 120);
    for y in 0..120 {
        for x in 0..120 {
            let dx = x as f32 - 60.0;
            let dy = y as f32 - 60.0;
            if dx * dx + dy * dy <= 50.0 * 50.0 {
                dot.put_pixel(x, y, Rgba([255, 200, 60, 255]));
            }
        }
    }
    dot.save(format!("{dir}/dot.png")).expect("save dot");
    eprintln!("  wrote {dir}/dot.png (120x120 gold dot)");

    eprintln!("\nDone. Generated 5 images in {dir}/");
}

fn inside_rounded_rect(x: f32, y: f32, w: f32, h: f32, margin: f32, radius: f32) -> bool {
    let inner_x0 = margin;
    let inner_x1 = w - margin;
    let inner_y0 = margin;
    let inner_y1 = h - margin;

    // Clearly inside the body.
    if x >= inner_x0 + radius && x <= inner_x1 - radius && y >= inner_y0 && y <= inner_y1 {
        return true;
    }
    if x >= inner_x0 && x <= inner_x1 && y >= inner_y0 + radius && y <= inner_y1 - radius {
        return true;
    }

    // Check rounded corners.
    let corners = [
        (inner_x0 + radius, inner_y0 + radius),
        (inner_x1 - radius, inner_y0 + radius),
        (inner_x0 + radius, inner_y1 - radius),
        (inner_x1 - radius, inner_y1 - radius),
    ];
    for (cx, cy) in corners {
        let in_band_x = (x - cx).abs() <= radius;
        let in_band_y = (y - cy).abs() <= radius;
        if in_band_x && in_band_y {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= radius * radius {
                return true;
            }
        }
    }
    false
}
