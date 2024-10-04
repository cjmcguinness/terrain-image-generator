use actix_cors::Cors;
use actix_files::Files;
use actix_web::{post, web, App, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;
use uuid::Uuid;
use image::{self, ImageFormat};
use terrain_image_generator::{ clear_directory, modify_tilelevel_in_xml, geojson_to_svg, simplify_geojson };

// Structure of incoming data
#[derive(Deserialize)]
struct BoundingBox {
    ulx: f64,
    uly: f64,
    lrx: f64,
    lry: f64,
    zoom_level: i32,
    option: String,
}

// Structure of outgoing data
#[derive(Serialize)]
struct ImagePath {
    image_path: String,
}

//Function to generate the hillshade image
fn generate_hillshade(bbox: &BoundingBox, uuid: &str, image_dir: &str) -> Result<String, HttpResponse> {
    // File paths for the generated images
    let tif_path = format!("{}/{}.tif", image_dir, uuid); // Path to generated uuid.tif
    let shaded_tif_path = format!("{}/{}-shaded.tif", image_dir, uuid); // Path to shaded uuid-shaded.tif
    let shaded_png_path = format!("{}/{}-shaded.png", image_dir, uuid); // Path to shaded uuid-shaded.png

    // Step 1: Generate the TIFF file using bounding box
    let gdal_translate = Command::new("gdal_translate")
        .arg("-of")
        .arg("GTiff")
        .arg("-projwin")
        .arg(bbox.ulx.to_string())
        .arg(bbox.uly.to_string())
        .arg(bbox.lrx.to_string())
        .arg(bbox.lry.to_string())
        .arg("-projwin_srs")
        .arg("EPSG:4326")
        .arg("./xml-file/elevation.xml") // Path to elevation.xml
        .arg(&tif_path) // Output TIFF file
        .output();

    match gdal_translate {
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "gdal_translate failed with stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return Err(HttpResponse::InternalServerError().body("Error generating TIFF file"));
            }
        }
        Err(e) => {
            eprintln!("Error running gdal_translate: {:?}", e);
            return Err(HttpResponse::InternalServerError().body("Error generating TIFF file"));
        }
    }

    // Step 2: Generate the hillshaded TIFF file
    let gdaldem_hillshade = Command::new("gdaldem")
        .arg("hillshade")
        .arg("-az")
        .arg("45")
        .arg("-z")
        .arg("8")
        .arg("-compute_edges")
        .arg(&tif_path) // Input TIFF file
        .arg(&shaded_tif_path) // Output shaded TIFF file
        .output();

    match gdaldem_hillshade {
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "gdaldem hillshade failed with stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return Err(HttpResponse::InternalServerError().body("Error generating hillshaded TIFF file"));
            }
        }
        Err(e) => {
            eprintln!("Error running gdaldem hillshade: {:?}", e);
            return Err(HttpResponse::InternalServerError().body("Error generating hillshaded TIFF file"));
        }
    }

    // Convert the shaded TIFF to PNG
    let img = match image::open(&shaded_tif_path) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("Error opening shaded TIFF file: {:?}", e);
            return Err(HttpResponse::InternalServerError().body("Error processing shaded TIFF file"));
        }
    };

    if let Err(e) = img.save_with_format(&shaded_png_path, ImageFormat::Png) {
        eprintln!("Error saving shaded PNG file: {:?}", e);
        return Err(HttpResponse::InternalServerError().body("Error saving shaded PNG file"));
    }

    println!("Successfully generated hillshaded map at: {}", shaded_png_path);

    // Return the path of the generated image
    Ok(format!("http://127.0.0.1:8000/backend/generated-images/{}-shaded.png", uuid))
}

fn generate_contours(bbox: &BoundingBox, uuid: &str, image_dir: &str) -> Result<String, HttpResponse> {
    // File paths for the generated images
    let tif_path1 = format!("{}/{}-tif1.tif", image_dir, uuid);
    let tif_path2 = format!("{}/{}-tif2.tif", image_dir, uuid);
    let contour_geojson_path = format!("{}/{}-geojson.geojson", image_dir, uuid);
    let simplified_geojson_path = format!("{}/{}-simplified.geojson", image_dir, uuid);
    let contour_svg_path = format!("{}/{}-contour-svg.svg", image_dir, uuid);

    //Step 1: Generate the TIFF file using bounding box
    let gdal_translate = Command::new("gdal_translate")
        .arg("-of")
        .arg("GTiff")
        .arg("-projwin")
        .arg(bbox.ulx.to_string())
        .arg(bbox.uly.to_string())
        .arg(bbox.lrx.to_string())
        .arg(bbox.lry.to_string())
        .arg("-projwin_srs")
        .arg("EPSG:4326")
        .arg("./xml-file/elevation.xml") // Path to elevation.xml
        .arg(&tif_path1) // Output TIFF file
        .output();

    match gdal_translate {
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "gdal_translate failed with stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return Err(HttpResponse::InternalServerError().body("Error generating TIFF file"));
            }
        }
        Err(e) => {
            eprintln!("Error running gdal_translate: {:?}", e);
            return Err(HttpResponse::InternalServerError().body("Error generating TIFF file"));
        }
    }

    //Step 2: Reproject the tif
    let gdalwarp = Command::new("gdalwarp")
        .arg("-s_srs")
        .arg("EPSG:3857")
        .arg("-t_srs")
        .arg("EPSG:4326")
        .arg(&tif_path1)
        .arg(&tif_path2)
        .output();

    match gdalwarp {
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "gdalwarp failed with stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return Err(HttpResponse::InternalServerError().body("Error generating reprojected TIFF file"));
            }
        }
        Err(e) => {
            eprintln!("Error running gdalwarp: {:?}", e);
            return Err(HttpResponse::InternalServerError().body("Error generating reprojected TIFF file"));
        }
    }

    // Step 3: Generate GeoJSON
    let gdal_contour = Command::new("gdal_contour")
        .arg("-a")
        .arg("elev")
        .arg("-i")
        .arg("10.0")
        .arg("-f")
        .arg("GeoJSON")
        .arg(&tif_path2)
        .arg(&contour_geojson_path)
        .output();

    match gdal_contour {
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "gdal_contour failed with stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return Err(HttpResponse::InternalServerError().body("Error generating GeoJSON file"));
            }
        }
        Err(e) => {
            eprintln!("Error running gdal_contour: {:?}", e);
            return Err(HttpResponse::InternalServerError().body("Error generating GeoJSON file"));
        }
    }
    
  // New Step 4: Simplify the GeoJSON using native Rust function
  match simplify_geojson(&contour_geojson_path, &simplified_geojson_path, 0.000000001) {
    Ok(_) => println!("Successfully simplified GeoJSON"),
    Err(e) => {
        eprintln!("Error simplifying GeoJSON: {:?}", e);
        return Err(HttpResponse::InternalServerError().body("Error simplifying GeoJSON file"));
    }
}

// Step 5: Generate SVG from simplified GeoJSON
match geojson_to_svg(&simplified_geojson_path, &contour_svg_path) {
    Ok(_) => {
        println!("Successfully generated contour map at: {}", contour_svg_path);
        Ok(format!("http://127.0.0.1:8000/backend/generated-images/{}-contour-svg.svg", uuid))
    },
    Err(e) => {
        eprintln!("Error generating SVG: {:?}", e);
        Err(HttpResponse::InternalServerError().body("Error generating SVG file"))
    }
}
}

// POST handler that receives the bounding box and zoom level, processes it, and returns the image path for display
#[post("/api/image")]
async fn return_image_path(web::Json(bbox): web::Json<BoundingBox>) -> HttpResponse {
    let image_dir = "./generated-images";

    if let Err(e) = clear_directory(&image_dir) {
        eprintln!("Error clearing directory: {:?}", e);
        return HttpResponse::InternalServerError().body("Error preparing image directory");
    }

    let uuid = Uuid::new_v4().to_string();
    let zoom = bbox.zoom_level;

    if let Err(e) = modify_tilelevel_in_xml("./xml-file/elevation.xml", zoom) {
        eprintln!("Error modifying XML: {:?}", e);
        return HttpResponse::InternalServerError().body("Error configuring XML file");
    }

    match bbox.option.as_str() { 
        "hillshade" => match generate_hillshade(&bbox, &uuid, image_dir) {
            Ok(final_image_path) => {
                // Return the result as JSON
                HttpResponse::Ok().json(ImagePath {
                    image_path: final_image_path,
                })
            }
            Err(response) => response,
        },
        "contour" => match generate_contours(&bbox, &uuid, image_dir) {
            Ok(final_image_path) => {
                // Return the result as JSON
                HttpResponse::Ok().json(ImagePath {
                    image_path: final_image_path,
                })
            }
            Err(response) => response,
        },
        _ => {
            // Handle any unrecognized options
            eprintln!("Unknown option: {}", bbox.option);
            HttpResponse::BadRequest().body("Invalid option provided")
        }
    }
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if let Err(e) = clear_directory("./generated-images") {
        eprintln!("Error creating generated-images directory: {:?}", e);
        return Err(e);
    }
    if let Err(e) = fs::create_dir_all("./xml-file") {
        eprintln!("Error creating xml-file directory: {:?}", e);
        return Err(e);
    }

    HttpServer::new(|| {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allow_any_header()
            .allow_any_method()
            .expose_any_header();

        App::new()
            .wrap(cors)
            .service(return_image_path)
            .service(
                Files::new("/backend/generated-images", "./generated-images")
                    .show_files_listing()
            )
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}