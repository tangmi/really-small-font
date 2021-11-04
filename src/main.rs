use anyhow::{Context, Result};
use bitmap_font::TextStyle;
use embedded_graphics::{
    draw_target::{DrawTarget, DrawTargetExt},
    pixelcolor::BinaryColor,
    prelude::{OriginDimensions, Point, Size},
    primitives::{Circle, Line, PrimitiveStyleBuilder, Rectangle, StyledDrawable},
    text::Text,
    Drawable, Pixel,
};
use image::{ImageBuffer, Rgb, RgbImage};

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
            // draw a smiley
            let mut display = display.translated(Point::new(150, 20));
            Circle::new(Point::zero(), 15).draw_styled(&thin_line, &mut display)?;
            Line::new(Point::new(4, 4), Point::new(4, 5)).draw_styled(&thin_line, &mut display)?;
            Line::new(Point::new(10, 4), Point::new(10, 5))
                .draw_styled(&thin_line, &mut display)?;
            let mut clipped = display.clipped(&Rectangle::new(Point::new(0, 8), Size::new(15, 6)));
            Circle::new(Point::new(4, 3), 7).draw_styled(&thin_line, &mut clipped)?;
        }

        display.into_inner().save_with_format(
            format!("screenshots/example-{:?}.png", text_color).to_ascii_lowercase(),
            image::ImageFormat::Png,
        )?;
    }

    Ok(())
}

/// If `false`, draw the image using whole pixels instead of subpixels. This can be useful to debug output.
const USE_SUBPIXEL: bool = true;

struct SubpixelImageBuffer {
    lit_pixel: Rgb<u8>,
    buffer: RgbImage,
}

impl SubpixelImageBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            lit_pixel: perceived_brightness::evenly_lit_pixel(),
            buffer: ImageBuffer::new(
                if USE_SUBPIXEL {
                    let extra = if width % 3 != 0 { 1 } else { 0 };
                    width / 3 + extra
                } else {
                    width
                },
                height,
            ),
        }
    }

    pub fn into_inner(self) -> RgbImage {
        self.buffer
    }
}

impl OriginDimensions for SubpixelImageBuffer {
    fn size(&self) -> Size {
        let (w, h) = self.buffer.dimensions();
        Size {
            width: if USE_SUBPIXEL { w * 3 } else { w },
            height: h,
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

            if USE_SUBPIXEL {
                let pixel = self.buffer.get_pixel_mut(x / 3, y);
                let subpixel = pos.x as usize % 3;

                // Assume pixel geometry is RGB
                // TODO: Do we need to adjust the brightness if multiple subpixels are lit?
                pixel.0[subpixel] = match color {
                    BinaryColor::Off => 0,
                    BinaryColor::On => self.lit_pixel[subpixel],
                };
            } else {
                self.buffer.put_pixel(
                    x,
                    y,
                    match color {
                        BinaryColor::Off => Rgb([0, 0, 0]),
                        BinaryColor::On => self.lit_pixel,
                    },
                )
            }
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
        (R_COEFFICIENT * r.powi(2) + G_COEFFICIENT * g.powi(2) + B_COEFFICIENT * b.powi(2)).sqrt()
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

        // Hand-wave-y gamma correction to make the colors look even a little more uniform. This probably is an issue with the perceptual brightness coefficients rather than actual color space issues?
        // let r = r.powf(2.2);
        // let g = g.powf(2.2);

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
