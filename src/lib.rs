use image;
use log;
use worker::*;

fn query(req: &Request, key: &str) -> Option<String> {
    req.url()
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.to_string())
}

#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    if req.method() != Method::Get {
        return Response::error("Method Not Allowed".to_string(), 405);
    }

    let width = match query(&req, "width") {
        Some(width) => width,
        None => {
            return Response::error("width is required".to_string(), 400);
        }
    };
    let height = match query(&req, "height") {
        Some(height) => height,
        None => {
            return Response::error("width is required".to_string(), 400);
        }
    };

    let bucket = match env.bucket("BUCKET") {
        Ok(bucket) => bucket,
        Err(e) => {
            log::error!("failed to get bucket: {e}");
            return Response::error("failed to get bucket", 500);
        }
    };
    let raw_img = match bucket.get("icon.jpg").execute().await {
        Ok(raw_img) => match raw_img {
            Some(raw_img) => raw_img,
            None => {
                log::error!("img is not found");
                return Response::error("img is not found", 404);
            }
        },
        Err(e) => {
            log::error!("failed to get font: {e}");
            return Response::error("img is not found", 500);
        }
    };
    let raw_img = raw_img.body().unwrap().bytes().await.unwrap();
    let img = image::io::Reader::new(std::io::Cursor::new(raw_img))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();

    let img = img.resize_exact(
        width.parse().unwrap(),
        height.parse().unwrap(),
        image::imageops::FilterType::Lanczos3,
    );

    let mut buffer = std::io::Cursor::new(vec![]);
    match img.write_to(&mut buffer, image::ImageFormat::Png) {
        Ok(_) => {}
        Err(e) => {
            log::error!("failed to write image: {e}");
            return Response::error("failed to write image", 500);
        }
    }

    let resp = match Response::from_bytes(buffer.into_inner()) {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("failed to create response: {e}");
            return Response::error("failed to create response", 500);
        }
    };
    let mut headers = Headers::new();
    match headers.set("content-type", "image/png") {
        Ok(_) => {}
        Err(e) => {
            log::error!("failed to set content-type header: {e}");
            return Response::error("failed to set content-type header", 500);
        }
    };
    match headers.set("Cache-Control", "public, max-age=604800") {
        // 1 week
        Ok(_) => {}
        Err(e) => {
            log::error!("failed to set Cache-Control header: {e}");
            return Response::error("failed to set Cache-Control header", 500);
        }
    };
    let resp = resp.with_headers(headers);

    Ok(resp)
}
