use std::{time::{Instant, Duration}, io::{Read, Write}, ops::Deref, fs::File, collections::HashMap, path::Path, sync::Arc};

use actix::{Actor, StreamHandler, AsyncContext, ActorContext};
use actix_web::{HttpRequest, web, Responder, get};
use actix_web_actors::ws::{self, ProtocolError};
use flate2::{write::GzEncoder, GzBuilder, Compression};
use futures_util::stream;
use notify::{Event, Watcher};
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};

use crate::WatchingData;

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,Serialize,Deserialize)]
enum PacketTypes {
    S2cHb,
    S2cSync,
    S2cSyncF(CCFsFile),

    C2sHb,
    C2sSync(CCFile),
    C2sComp(u32),
    C2sReady,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CCFsFile {
    name : String, // path
    data : String,
    read_only : bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CCPartialFile(CCFsFile, bool);
impl Default for CCPartialFile {
    fn default() -> Self {
        Self (CCFsFile::default(), false)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CCFile {
    Full(CCFsFile),
    Partial(CCPartialFile)
}
impl Deref for CCPartialFile {
    type Target = CCFsFile;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Default for CCFsFile {
    fn default() -> Self {
        Self {
            name : String::new(),
            data : String::new(),
            read_only : false,
        }
    }
}

fn create_gzs<T: ?Sized + Serialize>(tx : &T) -> Vec<u8>  {
    let mut writer = GzBuilder::new().write(Vec::new(), Compression::default());
    serde_json::to_writer(&mut writer, tx).unwrap();
    writer.finish().unwrap()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WsPacket {
    #[serde(rename = "p")]
    packet : PacketTypes
}
impl WsPacket {
    fn new(packet : PacketTypes) -> Self {
        Self {
            packet
        }
    }
    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

struct WsHandler {
    hb : Instant,
    parts : HashMap<String,File>,
    comp_id : u32,
    WatchData: Arc<RwLock<WatchingData>>
}
// impl Default for WsHandler {
//     fn default() -> Self {
//         Self {
//             hb : Instant::now(),
//             parts : HashMap::new(),
//             comp_id : 0
//         }
//     }
// }
impl WsHandler {
    fn new(watch_data: Arc<RwLock<WatchingData>>) -> Self {
        Self {
            hb : Instant::now(),
            parts : HashMap::new(),
            comp_id : 0,
            WatchData: watch_data
        }
    }
}
impl Actor for WsHandler {
    type Context = ws::WebsocketContext<Self>;
}

impl WsHandler {
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(Duration::new(5, 0), |act, ctx| {
            if Instant::now().duration_since(act.hb) > Duration::new(10, 0) {
                ctx.stop();
                ctx.close(None);
                println!("Websocket Client heartbeat failed, disconnecting!");
                return;
            }
            println!("Websocket Client heartbeat");

            ctx.binary(create_gzs(&WsPacket {
                packet : PacketTypes::S2cHb
            }));
        });
        ctx.run_interval(Duration::from_millis(500), |act,ctx| {
            let mut wd = act.WatchData.write();
            while let Ok(d) = wd.rx.try_recv() {
                println!("{:#?}", d);
                let begin = std::env::current_dir().unwrap().join(format!("./comp/{}", act.comp_id).as_str()).components().map(|_|()).collect::<Vec<()>>().len();
                let cc_path : String = d.paths.first().unwrap().components().skip(begin).map(|x| x.as_os_str().to_string_lossy().to_string()).collect::<Vec<String>>().join("/");
                match d.kind {
                    // notify::EventKind::Any => todo!(),
                    // notify::EventKind::Access(_) => todo!(),
                    notify::EventKind::Create(_) => {
                        let pather = d.paths.first().unwrap();
                        if pather.is_dir() {
                            let file = CCFsFile {
                                name : cc_path,
                                data: "dir".to_string(),
                                read_only : true
                            };
                            ctx.binary(create_gzs(&WsPacket {
                                packet : PacketTypes::S2cSyncF(file)
                            }));

                        } else {

                            let mut file = File::open(pather).unwrap();
                            let mut data = String::new();
                            file.read_to_string(&mut data).unwrap();
                            let file = CCFsFile {
                                name : cc_path,
                                data,
                                read_only : false
                            };
                            ctx.binary(create_gzs(&WsPacket {
                                packet : PacketTypes::S2cSyncF(file)
                            }));
                        }
                    },
                    notify::EventKind::Modify(_) => {
                        let pather = d.paths.first().unwrap();
                        if pather.is_dir() {
                            let file = CCFsFile {
                                name : cc_path,
                                data: "dir".to_string(),
                                read_only : true
                            };
                            ctx.binary(create_gzs(&WsPacket {
                                packet : PacketTypes::S2cSyncF(file)
                            }));

                        } else {

                            let pather = d.paths.first().unwrap();
                            let mut file = File::open(pather).unwrap();
                            let mut data = String::new();
                            file.read_to_string(&mut data).unwrap();
                            let file = CCFsFile {
                                name : cc_path,
                                data,
                                read_only : false
                            };
                            ctx.binary(create_gzs(&WsPacket {
                                packet : PacketTypes::S2cSyncF(file)
                            }));
                        }
                    },
                    notify::EventKind::Remove(_) => {
                        let pather = d.paths.first().unwrap();
                        let file = CCFsFile {
                            name : cc_path,
                            data : String::new(),
                            read_only : true
                        };
                        ctx.binary(create_gzs(&WsPacket {
                            packet : PacketTypes::S2cSyncF(file)
                        }));
                    },
                    // notify::EventKind::Other => todo!(),
                    _ => {}
                }
            }
        });
    }
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsHandler {
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        // println!("{:#?}", serde_json::to_string_pretty(&WsPacket::new(PacketTypes::C2sSync(CCFile::Partial(CCPartialFile::default())))));
        // println!("{:#?}", serde_json::to_string_pretty(&WsPacket::new(PacketTypes::C2sComp(String::from("test")))));

        ctx.binary(create_gzs(&WsPacket {
            packet : PacketTypes::S2cSync
        }));
    }
    fn finished(&mut self, ctx: &mut Self::Context) {
        let dat = Path::new(&format!("./comp/{}", self.comp_id));
        self.WatchData.write().watcher.unwatch(Path::new(&format!("./comp/{}", self.comp_id)));

    }

    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Binary(bin)) => {
                let res = (|bin:&[u8]| -> anyhow::Result<WsPacket> {
                    Ok(serde_json::from_slice::<WsPacket>(bin)?)
                    
                })(&bin[..]);
                if let Ok(packet) = res {
                    match packet.packet {
                        PacketTypes::C2sHb => {
                            println!("doki doki");
                            self.hb = Instant::now();
                        },
                        PacketTypes::C2sComp(x) => {
                            self.comp_id = x;
                            std::fs::create_dir_all(format!("./comp/{}", self.comp_id)).unwrap();
                        },
                        PacketTypes::C2sSync(f) => {
                            let com = format!("./comp/{}", self.comp_id);
                            let pather = Path::new(&com);
                            // println!("Syncing file: {:?}", f);
                            match f {
                                CCFile::Full(f) => {
                                    let path2 = pather.join(f.name.clone());
                                    if let Some(par) = path2.parent() {
                                        std::fs::create_dir_all(par).unwrap();
                                    }
                                    let mut file = File::create(path2).unwrap();
                                    file.write_all(f.data.as_bytes()).unwrap();
                                },
                                CCFile::Partial(f) => {
                                    let path2 = pather.join(f.name.clone());
                                    if let Some(par) = path2.parent() {
                                        std::fs::create_dir_all(par).unwrap();
                                    }
                                    if !self.parts.contains_key(&f.name) {
                                        println!("Creating file: {:?}", path2);
                                        self.parts.insert(f.name.clone(), File::create(&path2).unwrap());
                                    }
                                    let mut file = self.parts.get_mut(&f.name).unwrap();
                                    file.write_all(f.data.as_bytes()).unwrap();
                                    if f.1 {
                                        println!("Closing file: {:?}", path2);
                                        self.parts.remove(&f.name);
                                    }
                                }
                            }
                        }
                        PacketTypes::C2sReady => {
                            println!("client is ready to sync");
                            self.WatchData.write().watcher.watch(Path::new(&format!("./comp/{}", self.comp_id)), notify::RecursiveMode::Recursive);

                        }
                        _ => ()
                    }
                } else {
                    println!("Websocket Client sent invalid packet: {:?}", res);
                    ctx.stop();
                    ctx.close(None);
                }
            },
            _ => {
                println!("Websocket Client sent invalid packet: {:?}", msg);
            },
        }
    }
}
#[get("/ws")]
async fn ws_init(req: HttpRequest, stream: web::Payload, wd: web::Data<Arc<RwLock<WatchingData>>>) -> impl Responder {
    let resp = ws::start(WsHandler::new(Arc::clone(&*wd)), &req, stream);
    resp
}