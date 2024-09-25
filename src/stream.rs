use std::{
    path::Path,
    time::{Duration, Instant},
};

use ab_glyph::Font;
use anyhow::{anyhow, Context, Result};
use image::{DynamicImage, ImageBuffer, ImageFormat, Luma};
use show_image::{create_window, WindowOptions};
use tracing::{debug, info};

use crate::device::RemarkableStreamer;

const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;

const FONT_BYTES: &[u8] = include_bytes!("../static/Amazon-Ember-Medium.ttf");
const FONT_SIZE: f32 = 18.0;
// black
const TEXT_COLOR: image::Rgb<u8> = image::Rgb([0, 0, 0]);
const TEXT_MARGIN_PX: u32 = 10;

const MIN_DURATION_PER_FRAME: Duration = Duration::from_millis(100);

/// Stream the reMarkable tablet to the local screen.
///
/// Inspired by:
/// https://blog.owulveryck.info/2021/03/30/streaming-the-remarkable-2.html
pub fn stream(show_diagnostics: bool) -> Result<()> {
    info!("streaming reMarkable tablet");

    let rem = crate::device::Remarkable::open()?;

    let window = create_window(
        "reMarkable device stream",
        WindowOptions::default().set_size([HEIGHT as u32, WIDTH as u32]),
    )?;

    let font = ab_glyph::FontArc::try_from_slice(FONT_BYTES).context("failed to parse font")?;
    let scale = font
        .pt_to_px_scale(FONT_SIZE)
        .with_context(|| format!("failed to build PxScale from font size {FONT_SIZE}"))?;

    let streamer = rem.streamer()?;
    let mut frame_buffer = vec![0u8; HEIGHT * WIDTH];
    loop {
        let frame_begin = Instant::now();
        let mut image = get_frame(&streamer, &mut frame_buffer)?;

        if show_diagnostics {
            let frame_processing_duration = frame_begin.elapsed();
            let frame_rate = 1.0 / frame_processing_duration.as_secs_f32();
            let debug_text = format!(
                "frame latency: {}ms rate: {frame_rate:.2}fps",
                frame_processing_duration.as_millis()
            );

            let (text_width, text_height) =
                imageproc::drawing::text_size(scale, &font, &debug_text);

            let x = image.width() - text_width - TEXT_MARGIN_PX;
            let y = image.height() - text_height - TEXT_MARGIN_PX;
            imageproc::drawing::draw_text_mut(
                &mut image,
                TEXT_COLOR,
                x as _,
                y as _,
                scale,
                &font,
                &debug_text,
            );
        }

        window.set_image("image-001", image)?;

        let frame_duration = frame_begin.elapsed();
        debug!("frame latency: {frame_duration:?}");

        if frame_duration < MIN_DURATION_PER_FRAME {
            debug!("sleeping for {:?}", MIN_DURATION_PER_FRAME - frame_duration);
            std::thread::sleep(MIN_DURATION_PER_FRAME - frame_duration);
        }
    }
}

pub fn grab_frame(dest_file: impl AsRef<Path>) -> Result<()> {
    let rem = crate::device::Remarkable::open()?;
    let streamer = rem.streamer()?;
    let mut frame_buffer = vec![0u8; HEIGHT * WIDTH];
    let image = get_frame(&streamer, &mut frame_buffer)?;

    let ext = dest_file.as_ref().extension();
    let fmt =
        ImageFormat::from_extension(ext.ok_or_else(|| anyhow!("Image file extension required"))?)
            .ok_or_else(|| anyhow!("File extension {:?} invalid", ext))?;
    let mut f = std::fs::File::create(dest_file)?;
    image.write_to(&mut f, fmt)?;
    Ok(())
}

fn get_frame(
    streamer: &RemarkableStreamer,
    frame_buffer: &mut Vec<u8>,
) -> Result<ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
    let bytes = streamer.frame_buffer()?;

    ////////////////////////////////////////////////////////////////
    // Old code that used ffmpeg to do the RAW video to image conversion,
    // which is now done directly in Rust instead.
    //
    // let output = Command::new("ffmpeg")
    //     .arg("-vcodec")
    //     .arg("rawvideo")
    //     .arg("-f")
    //     .arg("rawvideo")
    //     .arg("-pixel_format")
    //     .arg(PIXEL_FORMAT)
    //     .arg("-video_size")
    //     .arg(format!("{WIDTH},{HEIGHT}"))
    //     .arg("-i")
    //     .arg("test.raw")
    //     .arg("-vf")
    //     .arg("colorlevels=rimin=0:rimax=29/255:gimin=0:gimax=29/255:bimin=0:bimax=29/255,transpose=3")
    //     .arg("-dframes")
    //     .arg("1")
    //     .arg("converted%d.png")
    //     .output()?;
    ////////////////////////////////////////////////////////////////

    let image_buffer_begin = Instant::now();
    for (i, pixel) in bytes.into_iter().step_by(2).enumerate() {
        if i >= frame_buffer.len() {
            break;
        }

        frame_buffer[i] = (pixel as f32 / 30.0 * 255.0) as u8;
    }

    debug!(
        "byte buffer to pixel buffer latency: {:?}",
        image_buffer_begin.elapsed()
    );

    let buffer =
        ImageBuffer::<Luma<u8>, Vec<u8>>::from_vec(WIDTH as _, HEIGHT as _, frame_buffer.clone())
            .unwrap();
    let image = DynamicImage::ImageLuma8(buffer)
        .rotate270()
        .fliph()
        .to_rgb8();
    debug!(
        "pixel buffer to ImageBuffer latency: {:?}",
        image_buffer_begin.elapsed()
    );

    Ok(image)
}
