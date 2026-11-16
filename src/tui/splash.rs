//! Startup splash: the KGB horse animation, drawn with half-block cells (two
//! square pixels per cell) so it works in any true-color terminal.

use std::io::Cursor;
use std::time::{Duration, Instant};

use image::codecs::gif::GifDecoder;
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, Rgba, RgbaImage};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

const GIF: &[u8] = include_bytes!("../../assets/kgb_horse_loading_animation.gif");

pub struct Splash {
    frames: Vec<(RgbaImage, Duration)>,
    started: Instant,
    /// Fill around the image: the average of the first frame's plain top-left corner.
    bg: Color,
    /// Frames scaled to `.0` pixels square, rebuilt when the terminal resizes.
    scaled: Option<(u32, Vec<RgbaImage>)>,
}

impl Splash {
    pub fn new() -> anyhow::Result<Self> {
        let frames = GifDecoder::new(Cursor::new(GIF))?
            .into_frames()
            .collect_frames()?
            .into_iter()
            .map(|f| {
                let delay = Duration::from(f.delay());
                (f.into_buffer(), delay)
            })
            .collect::<Vec<_>>();
        let bg = frames.first().map_or(Color::Reset, |(img, _)| corner_average(img));
        Ok(Self {
            frames,
            started: Instant::now(),
            bg,
            scaled: None,
        })
    }

    /// The frame to show now, or `None` once the animation has played through.
    fn current(&self) -> Option<usize> {
        let mut elapsed = self.started.elapsed();
        for (i, (_, delay)) in self.frames.iter().enumerate() {
            if elapsed < *delay {
                return Some(i);
            }
            elapsed -= *delay;
        }
        None
    }

    pub fn done(&self) -> bool {
        self.current().is_none()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let Some(i) = self.current() else { return };
        let bg = self.bg;
        buf.set_style(area, Style::new().bg(bg));

        // Square image, square half-block pixels: fit the smaller dimension.
        let size = u32::from(area.width).min(u32::from(area.height) * 2);
        if size == 0 {
            return;
        }
        if self.scaled.as_ref().is_none_or(|(s, _)| *s != size) {
            let scaled = self
                .frames
                .iter()
                .map(|(img, _)| imageops::resize(img, size, size, FilterType::Triangle))
                .collect();
            self.scaled = Some((size, scaled));
        }
        let img = &self.scaled.as_ref().unwrap().1[i];

        let x0 = area.x + (area.width - size as u16) / 2;
        let y0 = area.y + (area.height - size.div_ceil(2) as u16) / 2;
        for py in (0..size).step_by(2) {
            for px in 0..size {
                let top = rgb(img.get_pixel(px, py));
                let bottom = if py + 1 < size {
                    rgb(img.get_pixel(px, py + 1))
                } else {
                    bg
                };
                buf[(x0 + px as u16, y0 + (py / 2) as u16)]
                    .set_symbol("▀")
                    .set_fg(top)
                    .set_bg(bottom);
            }
        }
    }
}

fn corner_average(img: &RgbaImage) -> Color {
    let n = 32.min(img.width()).min(img.height());
    let mut sum = [0u32; 3];
    for y in 0..n {
        for x in 0..n {
            let p = img.get_pixel(x, y);
            for c in 0..3 {
                sum[c] += u32::from(p[c]);
            }
        }
    }
    let avg = |c: usize| (sum[c] / (n * n).max(1)) as u8;
    Color::Rgb(avg(0), avg(1), avg(2))
}

fn rgb(p: &Rgba<u8>) -> Color {
    Color::Rgb(p[0], p[1], p[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_embedded_gif() {
        let splash = Splash::new().unwrap();
        assert_eq!(splash.frames.len(), 12);
        assert!(!splash.done());
    }
}
