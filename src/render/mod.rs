use color::to_pdf_color;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use tracing::{debug, info, trace};

use crate::model;
use crate::model::content::{BrushType, Version};

mod color;

pub fn render_pdf<F: AsRef<Path>>(
    notebook: model::content::Notebook,
    page_filter: Box<dyn Fn(usize) -> bool>,
    output_file: F,
) {
    let page_width = Mm(model::WIDTH_PIXELS as _);
    let page_height = Mm(model::HEIGHT_PIXELS as _);
    let layer_name = "Layer 1";

    let (doc, page1, layer1) = PdfDocument::new(
        "printpdf graphics test",
        page_width,
        page_height,
        layer_name,
    );
    let black = Color::Greyscale(Greyscale::new(0.0, None));

    let mut current_layer = doc.get_page(page1).get_layer(layer1);

    for (idx, page) in notebook.pages.into_iter().enumerate() {
        let mut cumulative_thickness = 0.0;
        let mut point_count = 0;

        if !page_filter(idx) {
            continue;
        }

        // draw the lines
        for layer in page.layers {
            for line in layer.lines {
                let should_draw = match line.brush_type {
                    BrushType::Eraser | BrushType::EraserArea => false,
                    _ => true,
                };
                if !should_draw {
                    continue;
                }

                let pdf_color = to_pdf_color(line.color);
                current_layer.set_fill_color(pdf_color.clone());
                current_layer.set_outline_color(pdf_color.clone());

                for segment in line.points.windows(2) {
                    let x0 = segment[0].x as f64;
                    let y0 = model::HEIGHT_PIXELS as f64 - segment[0].y as f64;
                    let x1 = segment[1].x as f64;
                    let y1 = model::HEIGHT_PIXELS as f64 - segment[1].y as f64;

                    let points = vec![
                        (Point::new(Mm(x0), Mm(y0)), false),
                        (Point::new(Mm(x1), Mm(y1)), false),
                    ];

                    let line1 = Line {
                        points: points.clone(),
                        is_closed: true,
                        has_fill: true,
                        has_stroke: true,
                        is_clipping_path: false,
                    };

                    // for V6 the width per point / line segment is
                    // very noisy.  there are random segments that render
                    // extremely thin, segments that do not show up in the
                    // source or screen rendering.  it seems that there is either
                    // some alternate interpretation of the width field for certain
                    // points (e.g. width values below 1.0 should be ignored or something)
                    // or screen rendering is applying some smoothing process to line
                    // widths.  to account for this, here we naively just render each
                    // line as the max width observed across all points.  we likely lose
                    // some visual specificity here (you can't draw a single line that
                    // changes in thickness through the line) but it renders much better
                    // than rendering the point widths unmodified.
                    //
                    // TODO: is there a better approach to smoothing?
                    // TODO: should this be done in parsing?
                    let effective_thickness = match page.version {
                        Version::V3 => segment[0].width,
                        Version::V5 => segment[0].width,
                        Version::V6 => {
                            let mut max_over_points: f32 = 0.0;
                            for point in &line.points {
                                max_over_points = max_over_points.max(point.width);
                            }
                            max_over_points * 4.0
                        }
                    };

                    debug!(
                        "rendering point {:?} at thickness {} / {} => {}",
                        points, segment[0].width, segment[1].width, effective_thickness
                    );
                    current_layer.set_outline_thickness(effective_thickness as _);
                    current_layer.add_shape(line1);

                    cumulative_thickness += segment[0].width;
                    point_count += 1;
                }

                current_layer.set_fill_color(color::PDF_BLACK);
                current_layer.set_outline_color(color::PDF_BLACK);
            }
        }

        // indicate the notebook and page ID in the bottom left corner.  this is helpful
        // for debugging.  x is from left edge, y is from bottom edge.
        let text = format!("notebook: {}, page: {}", notebook.id, page.id);
        let font = doc.add_builtin_font(BuiltinFont::Courier).unwrap();
        current_layer.use_text(text, 48.0, Mm(10.0), Mm(10.0), &font);

        let (next_page, next_layer) = doc.add_page(page_width, page_height, layer_name);
        current_layer = doc.get_page(next_page).get_layer(next_layer);
        current_layer.set_fill_color(black.clone());
        current_layer.set_outline_color(black.clone());

        let avg_thickness = cumulative_thickness / point_count as f32;
        info!("page stats: points={point_count}, cumulative_thickness={cumulative_thickness}, avg_thickness={avg_thickness}");
    }

    trace!("writing to output path: {:?}", output_file.as_ref());
    std::fs::create_dir_all(output_file.as_ref().parent().unwrap()).unwrap();
    doc.save(&mut BufWriter::new(
        File::create(output_file.as_ref()).unwrap(),
    ))
    .unwrap();
}
