use actix_web::{HttpServer, web, App, Error, HttpRequest, Responder, HttpResponse};
use futures::future::{Future, ok};
use futures::Stream;

fn delete_file(info: web::Path<(String, )>) -> impl Responder {
    HttpResponse::Found()
}

fn download_file(info: web::Path<(String, )>) -> impl Responder {
    HttpResponse::Found()
}

fn upload_specified_file(payload: web::Payload, info: web::Path<(String, )>) -> impl Future<Item=HttpResponse, Error=Error> {
    payload
        .map_err(Error::from)
        .fold(web::BytesMut::new(), move |mut body, chunk| {
            Ok::<_, Error>(body)
        })
        .and_then(move |contents| {
            ok(HttpResponse::Ok().finish())
        })
}

fn upload_new_file(payload: web::Payload, info: web::Path<(String, )>) -> impl Future<Item=HttpResponse, Error=Error> {
    payload
        .map_err(Error::from)
        .fold(web::BytesMut::new(), move |mut body, chunk| {
            Ok::<_, Error>(body)
        })
        .and_then(move |contents| {
            ok(HttpResponse::Ok().finish())
        })
}

fn invalid_resource(req: HttpRequest) -> impl Responder {
    println!("Invalid URI: \"{}\"", req.uri());
    HttpResponse::NotFound()
}

fn main() -> std::io::Result<()> {
    let server_address = "127.0.0.1:8080";
    println!("Listening at address {}...", server_address);
    HttpServer::new(|| {
        App::new().service(
            web::resource("/{filename}")
                .route(web::delete().to(delete_file))
                .route(web::get().to(download_file))
                .route(web::put().to_async(upload_specified_file))
                .route(web::post().to_async(upload_new_file)),
        ).default_service(web::route().to(invalid_resource))
    }).bind(server_address)?.run()
}
