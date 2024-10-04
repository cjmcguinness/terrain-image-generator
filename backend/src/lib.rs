use std::{fs, io};
use regex::Regex;
use svg::node::element::path::Data;
use svg::node::element::Path;
use svg::Document;
use geojson::{GeoJson, Value, Geometry, FeatureCollection, Feature};
use geo_types::{ LineString, Coord };
use geo::SimplifyVw;      


// Helper function to clear a directory
pub fn clear_directory(path: &str) -> io::Result<()> {
    if fs::metadata(path).is_ok() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(())
}

// Function to modify the TileLevel in the XML file
pub fn modify_tilelevel_in_xml(xml_file: &str, zoom_level: i32) -> io::Result<()> {
    let contents = fs::read_to_string(xml_file)?;
    let re = Regex::new(r"<TileLevel>\d+</TileLevel>").unwrap();
    let new_contents = re.replace(&contents, format!("<TileLevel>{}</TileLevel>", zoom_level));
    fs::write(xml_file, new_contents.as_bytes())?;
    Ok(())
}

pub fn geojson_to_svg(input_geojson_path: &str, output_svg_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let geojson_str = fs::read_to_string(input_geojson_path)?;
    let geojson: GeoJson = geojson_str.parse()?;
    
    //Calculate bounding box of all coordinates
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    if let GeoJson::FeatureCollection(ref collection) = geojson {
        for feature in &collection.features {
            if let Some(geometry) = &feature.geometry {
                if let Value::LineString(coords) = &geometry.value {
                    for coord in coords {
                        min_x = min_x.min(coord[0]);
                        min_y = min_y.min(coord[1]);
                        max_x = max_x.max(coord[0]);
                        max_y = max_y.max(coord[1]);
                    }
                }
            }
        }
    }

    // Calculate scaling factors
    let width = 1000.0;
    let height = 600.0;
    let scale_x = width / (max_x - min_x);
    let scale_y = height / (max_y - min_y);

    // Prepare SVG
    let mut document = Document::new()
        .set("viewBox", (0, 0, width as u32, height as u32))
        .set("width", width)
        .set("height", height);
    
    // Iterate through the GeoJSON to get the contours
    if let GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            if let Some(geometry) = feature.geometry {
                // Extract LineString from the GeoJSON geometry
                if let Value::LineString(coords) = geometry.value {
                    // Create SVG path data from LineString coordinates
                    let mut data = Data::new();
                    let mut first = true;
                    for coord in coords {
                        let x = (coord[0] - min_x) * scale_x;
                        let y = height - (coord[1] - min_y) * scale_y; // Flip y-axis
                        if first {
                            data = data.move_to((x, y));
                            first = false;
                        } else {
                            data = data.line_to((x, y));
                        }
                    }
                    
                    let path = Path::new()
                        .set("fill", "none")
                        .set("stroke", "black")
                        .set("stroke-width", 1)
                        .set("d", data);
                    
                    document = document.add(path);
                }
            }
        }
    }
    
    // Write SVG to the file
    svg::save(output_svg_path, &document)?;
    Ok(())
}

pub fn simplify_geometry(geometry: Geometry, tolerance: f64) -> Geometry {
    match geometry.value {
        Value::LineString(coords) => {
            let line_string: LineString<f64> = coords.into_iter()
                .map(|coord| Coord { x: coord[0], y: coord[1] })
                .collect();
            let simplified = line_string.simplify_vw(&tolerance);
            let simplified_coords: Vec<Vec<f64>> = simplified.into_iter()
                .map(|coord| vec![coord.x, coord.y])
                .collect();
            Geometry::new(Value::LineString(simplified_coords))
        },
        
        _ => geometry,
    }
}

pub fn simplify_geojson(input_path: &str, output_path: &str, tolerance: f64) -> Result<(), Box<dyn std::error::Error>> {
    // Read the GeoJSON file
    let geojson_str = std::fs::read_to_string(input_path)?;
    let geojson: FeatureCollection = geojson_str.parse()?;

    // Simplify each feature
    let simplified_features: Vec<Feature> = geojson.features.into_iter().map(|feature| {
        let simplified_geometry = simplify_geometry(feature.geometry.unwrap(), tolerance);
        Feature {
            geometry: Some(simplified_geometry),
            ..feature
        }
    }).collect();

    // Create a new FeatureCollection with simplified features
    let simplified_collection = FeatureCollection {
        features: simplified_features,
        bbox: geojson.bbox,
        foreign_members: geojson.foreign_members,
    };

    // Write the simplified GeoJSON to file
    let simplified_geojson_str = serde_json::to_string(&simplified_collection)?;
    std::fs::write(output_path, simplified_geojson_str)?;

    Ok(())
}