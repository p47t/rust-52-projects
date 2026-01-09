use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use futures::stream::StreamExt; // Removed TryStreamExt and future::{Future, ok}
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Define the shared store type alias
type FileStore = Arc<Mutex<HashMap<String, web::Bytes>>>;

async fn delete_file(path: web::Path<String>, store: web::Data<FileStore>) -> impl Responder {
    let filename = path.into_inner();
    let mut store_guard = store.lock().unwrap();
    if store_guard.remove(&filename).is_some() {
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::NotFound().finish()
    }
}

async fn download_file(path: web::Path<String>, store: web::Data<FileStore>) -> impl Responder {
    let filename = path.into_inner();
    let store_guard = store.lock().unwrap();
    match store_guard.get(&filename) {
        Some(file_bytes) => HttpResponse::Ok()
            .content_type("application/octet-stream")
            .body(file_bytes.clone()),
        None => HttpResponse::NotFound().finish(),
    }
}

async fn upload_specified_file(
    mut payload: web::Payload,
    path: web::Path<String>,
    store: web::Data<FileStore>,
) -> Result<HttpResponse, Error> {
    let filename = path.into_inner();
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        body.extend_from_slice(&chunk);
    }

    let mut store_guard = store.lock().unwrap();
    store_guard.insert(filename, body.freeze());
    Ok(HttpResponse::Ok().finish())
}

// upload_new_file is essentially the same as upload_specified_file for in-memory store
// as PUT and POST on the same resource path will just overwrite.
async fn upload_new_file(
    mut payload: web::Payload,
    path: web::Path<String>,
    store: web::Data<FileStore>,
) -> Result<HttpResponse, Error> {
    let filename = path.into_inner();
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        body.extend_from_slice(&chunk);
    }

    let mut store_guard = store.lock().unwrap();
    store_guard.insert(filename, body.freeze());
    Ok(HttpResponse::Ok().finish())
}

async fn invalid_resource(req: HttpRequest) -> impl Responder {
    println!("Invalid URI: \"{}\"", req.uri());
    HttpResponse::NotFound()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let server_address = "127.0.0.1:8080";
    println!("Listening at address {}...", server_address);

    // Initialize the shared file store
    let file_store_data =
        web::Data::new(Arc::new(Mutex::new(HashMap::<String, web::Bytes>::new())));

    HttpServer::new(move || {
        // Closure needs to be move due to file_store_data
        App::new()
            .app_data(file_store_data.clone()) // Add shared state
            .service(
                web::resource("/{filename}")
                    .route(web::delete().to(delete_file))
                    .route(web::get().to(download_file))
                    .route(web::put().to(upload_specified_file))
                    .route(web::post().to(upload_new_file)),
            )
            .default_service(web::route().to(invalid_resource))
    })
    .bind(server_address)?
    .run()
    .await
}
