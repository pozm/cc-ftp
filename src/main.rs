use std::{env::args, sync::Arc};

use actix_files::{NamedFile, Files};
use actix_web::{HttpServer, web::{self, service, Data}, App, Responder, get, HttpRequest, HttpResponse};
use cc_ftp::{ws::ws_init, WatchingData};
use notify::{recommended_watcher, ReadDirectoryChangesWatcher, Event};
use parking_lot::RwLock;

#[get("/init.lua")]
async fn init_lua() -> impl Responder {
    NamedFile::open("./lua/init.lua")
}
#[get("/start")]
async fn start(req:HttpRequest) -> impl Responder {
    if let Some(addr) = req.headers().get("host") {
        HttpResponse::Ok().body(format!("shell.run('wget run http://{0}/init.lua {0}')",addr.to_str().unwrap()))
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()>  {
    let (btx,brx) = tokio::sync::broadcast::channel::<Event>(10);
    let mut watcher = notify::recommended_watcher(move |res| {
        match res {
            Ok(event) => {
                // println!("event: {:?}", event);
                btx.send(event);
            },
            Err(e) => {
                println!("watch error: {:?}", e);
            }
        }
    }).expect("unable to start watcher");
    let wdata = Arc::new(RwLock::new(WatchingData {
        watcher,
        rx : brx
    }));
    HttpServer::new(move || {
        App::new()
        .app_data(Data::new(wdata.clone()))
        .service(init_lua)
        .service(start)
        .service(ws_init)
        .service(Files::new("/deps", "./lua/deps"))
    })
    .bind(("0.0.0.0", args().nth(1).unwrap_or("8080".to_string()).parse().unwrap()))?
    .run()
    .await
}
