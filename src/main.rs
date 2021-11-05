use anyhow::Result;
use bitmap_font::TextStyle;
use embedded_graphics::{
    draw_target::{DrawTarget, DrawTargetExt},
    pixelcolor::BinaryColor,
    prelude::{Point, Size},
    primitives::{Circle, Line, PrimitiveStyleBuilder, Rectangle, StyledDrawable},
    text::Text,
    Drawable,
};

use crate::subpixel_image_buffer::SubpixelImageBuffer;

fn main() -> Result<()> {
    for text_color in [BinaryColor::On, BinaryColor::Off] {
        let mut display = SubpixelImageBuffer::new(200, 50);

        // let style = MonoTextStyle::new(&embedded_graphics::mono_font::ascii::FONT_4X6, text_color); // smaller font, but harder to read
        let style = TextStyle::new(&bitmap_font::tamzen::FONT_5x9, text_color);

        display.clear(text_color.invert())?;

        Text::new("ABCDEF", Point::new(3, 9), style).draw(&mut display)?;
        Text::new("This is a little test!", Point::new(3, 20), style).draw(&mut display)?;

        let thin_line = PrimitiveStyleBuilder::new()
            .stroke_width(1)
            .stroke_color(text_color)
            .build();

        Line::new(Point::new(0, 0), Point::new(3, 3)).draw_styled(&thin_line, &mut display)?;

        {
            // draw a many lines, which will turn into solid blocks of color. these should all have the same brightness.
            let mut display = display.translated(Point::new(9, 40));
            let mut i = 0;
            for _ in 0..3 {
                for _ in 0..5 {
                    Line::new(Point::new(i, 0), Point::new(i, 6))
                        .draw_styled(&thin_line, &mut display)?;
                    // advance by 3 to keep the same subpixel
                    i += 3;
                }
                // advance by 1 to switch to the next subpixel
                i += 1;
            }

            // draw 3 two-pixel-wide lines at different offsets. these should all have the same thickness.
            i += 12;
            let med_line = PrimitiveStyleBuilder::new()
                .stroke_width(2)
                .stroke_color(text_color)
                .build();
            for _ in 0..3 {
                Line::new(Point::new(i, 0), Point::new(i, 6))
                    .draw_styled(&med_line, &mut display)?;
                i += 7;
            }

            // do the same with a three-pixel-wide line. these should all have the same thickness.
            i += 10;
            let thick_line = PrimitiveStyleBuilder::new()
                .stroke_width(3)
                .stroke_color(text_color)
                .build();
            for _ in 0..3 {
                Line::new(Point::new(i, 0), Point::new(i, 6))
                    .draw_styled(&thick_line, &mut display)?;
                i += 7;
            }
        }

        {
            // draw a smiley
            let mut display = display.translated(Point::new(150, 20));
            Circle::new(Point::zero(), 15).draw_styled(&thin_line, &mut display)?;
            Line::new(Point::new(4, 4), Point::new(4, 5)).draw_styled(&thin_line, &mut display)?;
            Line::new(Point::new(10, 4), Point::new(10, 5))
                .draw_styled(&thin_line, &mut display)?;
            let mut clipped = display.clipped(&Rectangle::new(Point::new(0, 8), Size::new(15, 6)));
            Circle::new(Point::new(4, 3), 7).draw_styled(&thin_line, &mut clipped)?;
        }

        display.to_non_subpixel_image().save_with_format(
            format!("screenshots/example-big-{:?}.png", text_color).to_ascii_lowercase(),
            image::ImageFormat::Png,
        )?;

        display.into_inner().save_with_format(
            format!("screenshots/example-{:?}.png", text_color).to_ascii_lowercase(),
            image::ImageFormat::Png,
        )?;
    }

    Ok(())
}

mod subpixel_image_buffer {
    use anyhow::Context;
    use embedded_graphics::{
        draw_target::DrawTarget,
        pixelcolor::BinaryColor,
        prelude::{OriginDimensions, Size},
        Pixel,
    };
    use image::{ImageBuffer, Rgb, RgbImage};

    /// A monochromatic buffer that will emit an image that is 1/3 of its reported width. It does so by treating subpixels as pixels.
    pub struct SubpixelImageBuffer {
        /// Pixel that contains the max value per channel we want to write out.
        reference_pixel: Rgb<u8>,

        /// The actual width is 1/3 (rounded up) of what this struct reports.
        buffer: RgbImage,
    }

    impl SubpixelImageBuffer {
        /// Note: rounds up `width` to the nearest multiple of 3
        pub fn new(width: u32, height: u32) -> Self {
            let extra = if width % 3 != 0 { 1 } else { 0 };
            Self {
                reference_pixel: perceived_brightness::evenly_lit_pixel(),
                buffer: ImageBuffer::new(width / 3 + extra, height),
            }
        }

        pub fn into_inner(self) -> RgbImage {
            self.buffer
        }

        /// Draw the image using whole pixels instead of subpixels. This can be useful to debug output.
        pub fn to_non_subpixel_image(&self) -> RgbImage {
            let mut pixel_buffer: RgbImage =
                ImageBuffer::new(self.buffer.width() * 3, self.buffer.height() * 3);

            for (i, pixel) in self.buffer.pixels().enumerate() {
                let x = (i as u32 % self.buffer.width()) * 3;
                let y = (i as u32 / self.buffer.width()) * 3;

                for y in y..y + 3 {
                    for channel in 0..3 {
                        if pixel.0[channel] != 0 {
                            let mut output = Rgb([0, 0, 0]);
                            output.0[channel] = pixel.0[channel];
                            pixel_buffer.put_pixel(x + channel as u32, y, output);
                        }
                    }
                }
            }

            pixel_buffer
        }

        fn put_pixel(&mut self, x: u32, y: u32, val: f64) {
            let pixel = self.buffer.get_pixel_mut(x / 3, y);
            let subpixel = x as usize % 3;

            // Assume pixel geometry is RGB
            // TODO: Do we need to adjust the brightness if multiple subpixels are lit?
            let value_normalized = self.reference_pixel[subpixel] as f64 / 255.0 * val;

            // Hand-wave-y gamma correction to make the colors look even a little more uniform. This probably is an issue with the perceptual brightness coefficients rather than actual color space issues?
            const GAMMA_PER_CHANNEL: [f64; 3] = [2.2, 2.0, 1.8]; // Generated from observing the output of the build script.
            let value_normalized = value_normalized
                .powf(1.0 / 2.2) // to linear
                .powf(GAMMA_PER_CHANNEL[subpixel]); // to our custom gamma

            pixel.0[subpixel] = (value_normalized * 255.0) as u8;
        }
    }

    impl OriginDimensions for SubpixelImageBuffer {
        fn size(&self) -> Size {
            Size {
                width: self.buffer.width() * 3,
                height: self.buffer.height(),
            }
        }
    }

    impl DrawTarget for SubpixelImageBuffer {
        type Color = BinaryColor;

        type Error = anyhow::Error;

        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            for Pixel(pos, color) in pixels {
                let x = u32::try_from(pos.x).context("x position was negative")?;
                let y = u32::try_from(pos.y).context("y position was negative")?;
                self.put_pixel(
                    x,
                    y,
                    match color {
                        BinaryColor::Off => 0.0,
                        BinaryColor::On => 1.0,
                    },
                );
            }

            Ok(())
        }
    }

    mod perceived_brightness {
        use image::Rgb;

        // From https://alienryderflex.com/hsp.html
        const R_COEFFICIENT: f64 = 0.299;
        const G_COEFFICIENT: f64 = 0.587;
        const B_COEFFICIENT: f64 = 0.114;

        #[test]
        fn coefficient_sanity() {
            use assert_approx_eq::assert_approx_eq;

            assert_approx_eq!(1.0, R_COEFFICIENT + G_COEFFICIENT + B_COEFFICIENT);
        }

        fn perceived_brightness(color: [f64; 3]) -> f64 {
            let r = color[0] as f64;
            let g = color[1] as f64;
            let b = color[2] as f64;
            (R_COEFFICIENT * r.powi(2) + G_COEFFICIENT * g.powi(2) + B_COEFFICIENT * b.powi(2))
                .sqrt()
        }

        /// A color where each channel is approximately the same perceptual brightness.
        ///
        /// Should be a light purple.
        pub fn evenly_lit_pixel() -> Rgb<u8> {
            // The blue channel is the weakest, so we need to dim other channels to accommodate it
            let desired_brightness = perceived_brightness([0.0, 0.0, 1.0]);

            // Solve `perceived_brightness` for `r` and `g` when the other channels are 0.
            let r = (desired_brightness.powi(2) / R_COEFFICIENT).sqrt();
            let g = (desired_brightness.powi(2) / G_COEFFICIENT).sqrt();

            Rgb([(r * 255.0) as u8, (g * 255.0) as u8, 255])
        }

        /// Test that the `evenly_lit_pixel` is indeed evenly lit,
        #[test]
        fn sanity() {
            use assert_approx_eq::assert_approx_eq;

            let max_brightness = perceived_brightness([0.0, 0.0, 1.0]);
            let evenly_lit_pixel = evenly_lit_pixel();

            dbg!(evenly_lit_pixel);

            let threshold = 0.002;

            assert_approx_eq!(
                perceived_brightness([evenly_lit_pixel[0] as f64 / 255.0, 0.0, 0.0]),
                max_brightness,
                threshold
            );
            assert_approx_eq!(
                perceived_brightness([0.0, evenly_lit_pixel[1] as f64 / 255.0, 0.0]),
                max_brightness,
                threshold
            );
            assert_approx_eq!(
                perceived_brightness([0.0, 0.0, evenly_lit_pixel[2] as f64 / 255.0]),
                max_brightness,
                threshold
            );
        }
    }
}
