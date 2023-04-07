use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::model;
use crate::model::content::BrushType;

pub fn render_pdf<F: AsRef<Path>>(notebook: model::content::Notebook, output_file: F) {
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
    current_layer.set_fill_color(black.clone());
    current_layer.set_outline_color(black.clone());

    for page in notebook.pages {
        for layer in page.layers {
            for line in layer.lines {
                let should_draw = match line.brush_type {
                    BrushType::Eraser | BrushType::EraserArea => false,
                    _ => true,
                };
                if !should_draw {
                    continue;
                }

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
                        points: points,
                        is_closed: true,
                        has_fill: true,
                        has_stroke: true,
                        is_clipping_path: false,
                    };

                    current_layer.set_outline_thickness(segment[0].width as _);
                    current_layer.add_shape(line1);
                }
            }
        }

        let (next_page, next_layer) = doc.add_page(page_width, page_height, layer_name);
        current_layer = doc.get_page(next_page).get_layer(next_layer);
        current_layer.set_fill_color(black.clone());
        current_layer.set_outline_color(black.clone());
    }

    println!("writing to output path: {:?}", output_file.as_ref());
    std::fs::create_dir_all(output_file.as_ref().parent().unwrap()).unwrap();
    doc.save(&mut BufWriter::new(
        File::create(output_file.as_ref()).unwrap(),
    ))
    .unwrap();
}
