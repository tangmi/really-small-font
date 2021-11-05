use std::{convert::Infallible, env, path::PathBuf};

use embedded_graphics::{
    draw_target::{DrawTarget, DrawTargetExt},
    mono_font::MonoTextStyle,
    pixelcolor::{Rgb888, RgbColor},
    prelude::{OriginDimensions, Point, Size},
    primitives::{Line, PrimitiveStyleBuilder, Rectangle, StyledDrawable},
    text::Text,
    Drawable, Pixel,
};
use image::{Rgb, RgbImage};

/// Generate the gamma calibration image
fn main() {
    let section_width = 200;

    let mut display = RgbImageDrawTarget(RgbImage::new(section_width * 4, 600));

    let section_size = Size::new(section_width, display.size().height);

    for (color, crop_rect) in [
        (
            Rgb888::WHITE,
            Rectangle::new(Point::new(0, 0), section_size),
        ),
        (
            Rgb888::RED,
            Rectangle::new(Point::new(section_width as i32, 0), section_size),
        ),
        (
            Rgb888::GREEN,
            Rectangle::new(Point::new(2 * section_width as i32, 0), section_size),
        ),
        (
            Rgb888::BLUE,
            Rectangle::new(Point::new(3 * section_width as i32, 0), section_size),
        ),
    ] {
        let mut display = display.cropped(&crop_rect);

        let style = PrimitiveStyleBuilder::new()
            .stroke_color(color)
            .stroke_width(1)
            .build();

        let text_style = MonoTextStyle::new(
            &embedded_graphics::mono_font::ascii::FONT_6X10,
            Rgb888::WHITE,
        );

        for y in 0..display.size().height as i32 {
            if y % 2 == 0 {
                Line::new(
                    Point::new(0, y),
                    Point::new(display.size().width as i32 / 2, y),
                )
                .draw_styled(&style, &mut display)
                .unwrap();
            }

            let y_normalized = y as f64 / display.size().height as f64;
            let gamma_value = 1.0 + y_normalized * 2.0;
            Line::new(
                Point::new(display.size().width as i32 / 2, y),
                Point::new(display.size().width as i32 - 1, y),
            )
            .draw_styled(
                &PrimitiveStyleBuilder::new()
                    .stroke_color({
                        // Go through two "to linear" transforms, the first one gets us out of srgb and the second one "undoes" the space so we can apply our custom gamma value.
                        let half = 0.5_f64.powf(1.0 / 2.2).powf(1.0 / 2.2).powf(gamma_value);
                        Rgb888::new(
                            (half * color.r() as f64) as u8,
                            (half * color.g() as f64) as u8,
                            (half * color.b() as f64) as u8,
                        )
                    })
                    .stroke_width(1)
                    .build(),
                &mut display,
            )
            .unwrap();
        }

        let spacing = 15;
        for y in spacing / 2..display.size().height as i32 - spacing / 2 {
            if y % spacing == 0 {
                let y_normalized = y as f64 / display.size().height as f64;
                let gamma_value = 1.0 + y_normalized * 2.0;
                Text::with_baseline(
                    &format!("{:.2}", gamma_value),
                    Point::new(display.size().width as i32 * 3 / 4, y),
                    text_style,
                    embedded_graphics::text::Baseline::Middle,
                )
                .draw(&mut display)
                .unwrap();
            }
        }
    }

    let out_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("screenshots");
    display
        .0
        .save_with_format(out_dir.join("gamma.png"), image::ImageFormat::Png)
        .unwrap();
}

/// A monochromatic buffer that will emit an image that is 1/3 of its reported width. It does so by treating subpixels as pixels.
pub struct RgbImageDrawTarget(RgbImage);

impl OriginDimensions for RgbImageDrawTarget {
    fn size(&self) -> Size {
        Size {
            width: self.0.width(),
            height: self.0.height(),
        }
    }
}

impl DrawTarget for RgbImageDrawTarget {
    type Color = Rgb888;

    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(pos, color) in pixels {
            self.0.put_pixel(
                u32::try_from(pos.x).unwrap(),
                u32::try_from(pos.y).unwrap(),
                Rgb([color.r(), color.g(), color.b()]),
            );
        }

        Ok(())
    }
}
