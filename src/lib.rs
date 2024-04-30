use hex;
use image;
use log;
use sha2::{Digest, Sha256};
use worker::*;

struct Query {
    width: u32,
    height: u32,
    token: String,
}

impl Query {
    fn from_request(req: &Request) -> Result<Self> {
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        let mut token = String::new();

        req.url()
            .unwrap()
            .query_pairs()
            .for_each(|(k, v)| match k.as_ref() {
                "width" | "w" => width = v.parse().unwrap(),
                "height" | "h" => height = v.parse().unwrap(),
                "token" => token = v.to_string(),
                _ => {}
            });

        Ok(Self {
            width,
            height,
            token,
        })
    }

    fn full_path(&self) -> String {
        format!("/?width={}&height={}", self.width, self.height)
    }

    pub fn verify_token(&self, secret: String) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(self.full_path().as_bytes());
        hasher.update(b"$");
        hasher.update(secret.as_bytes());

        let mut buf = [0u8; 32];
        buf.copy_from_slice(&hasher.finalize());

        hex::encode(buf) == self.token
    }
}

#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    if req.method() != Method::Get {
        return Response::error("Method Not Allowed".to_string(), 405);
    }

    let url = match req.url() {
        Ok(url) => url,
        Err(e) => {
            log::error!("failed to get url: {e}");
            return Response::error("failed to get url", 500);
        }
    };
    let cache = Cache::default();
    let cache_key = CacheKey::Url(url.to_string());
    let cached = match cache.get(cache_key, false).await {
        Ok(cached) => cached,
        Err(e) => {
            log::error!("failed to get cache: {e}");
            None
        }
    };
    match cached {
        Some(cached) => {
            return Ok(cached);
        }
        None => {
            log::info!("cache not found (url = {url})");
        }
    }

    let query = match Query::from_request(&req) {
        Ok(query) => query,
        Err(e) => {
            log::error!("failed to parse query: {e}");
            return Response::error("failed to parse query", 400);
        }
    };
    // query の token が正しいか検証
    // 正しくない場合は query が改ざんされている可能性があるので 403 を返す
    match query.verify_token(env.secret("TOKEN_SECRET").unwrap().to_string()) {
        true => {}
        false => {
            log::error!("token is invalid");
            return Response::error("token is invalid", 403);
        }
    }

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
        query.width,
        query.height,
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
    let mut resp = resp.with_headers(headers);
    let cloned_resp = match resp.cloned() {
        Ok(cloned_resp) => cloned_resp,
        Err(e) => {
            log::error!("failed to clone response: {e}");
            return Response::error("failed to clone response", 500);
        }
    };
    match cache.put(url.to_string(), cloned_resp).await {
        Ok(_) => {}
        Err(e) => {
            // cache に保存できなくてもレスポンスは返す
            log::error!("failed to put cache: {e}");
        }
    };

    Ok(resp)
}
