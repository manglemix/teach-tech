use std::{collections::{hash_map::Entry, HashMap}, net::{IpAddr, SocketAddr}, sync::OnceLock};

use futures::{stream::FuturesUnordered, StreamExt};
use fxhash::{FxBuildHasher, FxHashMap};
use sea_orm::{prelude::*, ActiveValue};
use tokio::{io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter}, net::{tcp::{OwnedReadHalf, OwnedWriteHalf, ReuniteError}, TcpListener, TcpStream}, runtime::Handle, sync::Mutex};
use tracing::error;

use crate::{db::get_db, ApiConfig, TeachCore};

static CURRENT_ADDRESS: OnceLock<SocketAddr> = OnceLock::new();
const SIBLING_PORT: u16 = 22114;
static SIBLING_MESSAGE_HANDLERS: Mutex<Vec<Box<dyn FnMut(&str, &[u8]) + Send>>> = Mutex::const_new(vec![]);
static SIBLING_CONNS: Mutex<FxHashMap<IpAddr, BufWriter<OwnedWriteHalf>>> = Mutex::const_new(HashMap::with_hasher(FxBuildHasher::new()));


async fn handle_tcp_reader(mut reader: BufReader<OwnedReadHalf>, peer_ip: IpAddr) {
    tokio::spawn(async move {
        let mut buffer: Vec<u8> = vec![];
        loop {
            let source_size = match reader.read_u64().await {
                Ok(s) => s,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    error!("Failed to read source size from sibling {}: {}", peer_ip, e);
                    break;
                }
            };
            buffer.resize(source_size as usize, 0);
            match reader.read_exact(&mut buffer).await {
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    error!("Failed to read source from sibling {}: {}", peer_ip, e);
                    break;
                }
            }
            let data_size = match reader.read_u64().await {
                Ok(s) => s,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    error!("Failed to read data size from sibling {}: {}", peer_ip, e);
                    break;
                }
            };
            buffer.resize((source_size + data_size) as usize, 0);
            match reader.read_exact(&mut buffer[(source_size as usize)..]).await {
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    error!("Failed to read data from sibling {}: {}", peer_ip, e);
                    break;
                }
            }
            let Ok(source) = std::str::from_utf8(&buffer[..source_size as usize]) else {
                error!("Failed to parse source from sibling {}", peer_ip);
                continue;
            };
            for handler in SIBLING_MESSAGE_HANDLERS.lock().await.iter_mut() {
                handler(source, &buffer[(source_size as usize)..]);
            }
        }
        let reader = reader.into_inner();
        let mut conns = SIBLING_CONNS.lock().await;
        if let Some(mut writer) = conns.remove(&peer_ip) {
            let _ = writer.flush().await;
            let writer = writer.into_inner();
            // If the reunite fails, the writer belongs to a reconnection from the sibling
            if let Err(ReuniteError(_, writer)) = reader.reunite(writer) {
                conns.insert(peer_ip, BufWriter::new(writer));
            }
        }
    });
}


pub fn add_to_core<S: Clone + Send + Sync + 'static>(mut core: TeachCore<S>) -> anyhow::Result<TeachCore<S>> {
    struct OnDrop {
        server_address: SocketAddr
    }

    impl Drop for OnDrop {
        fn drop(&mut self) {
            Handle::current().block_on(async {
                if let Err(e) = Entity::delete_by_id(&self.server_address.to_string()).exec(get_db()).await {
                    error!("Failed to remove server address from database: {}", e);
                }
            });
        }
    }

    core.add_db_reset_config(Entity);
    let api_config: ApiConfig = toml::from_str(core.get_config_str())?;
    CURRENT_ADDRESS.set(api_config.server_address).expect("Server address is already initialized");
    core.add_to_drop(OnDrop {
        server_address: api_config.server_address
    });
    core.add_on_serve(move || async move {
        ActiveModel {
            address: ActiveValue::set(api_config.server_address.to_string())
        }.insert(get_db()).await?;
        let mut addr = api_config.server_address;
        addr.set_port(SIBLING_PORT);
        let listener = TcpListener::bind(addr).await?;
        tokio::spawn(async move {
            loop {
                let (stream, addr) = match listener.accept().await {
                    Ok(x) => x,
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                        continue;
                    }
                };
                let (reader, writer) = stream.into_split();
                let writer = BufWriter::new(writer);
                {
                    let mut conns = SIBLING_CONNS.lock().await;
                    conns.insert(addr.ip(), writer);
                }
                handle_tcp_reader(BufReader::new(reader), addr.ip()).await;
            }
        });
        Ok(())
    });
    Ok(core)
}

pub async fn send_to_siblings_raw(source: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let mut sibling_conns = SIBLING_CONNS.lock().await;
    let mut to_remove = vec![];
    {
        let mut futures = FuturesUnordered::new();
        for backend_data in Entity::find().all(get_db()).await?.into_iter() {
            if backend_data.address == CURRENT_ADDRESS.get().unwrap().to_string() {
                continue;
            }
            let mut addr: SocketAddr = match backend_data.address.parse() {
                Ok(x) => x,
                Err(e) => {
                    error!("Failed to parse address {}: {}", backend_data.address, e);
                    continue;
                }
            };
            addr.set_port(SIBLING_PORT);
            match sibling_conns.entry(addr.ip()) {
                Entry::Occupied(_) => {}
                Entry::Vacant(vacant_entry) => {
                    let stream = match TcpStream::connect(addr).await {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to connect to sibling {}: {}", addr, e);
                            continue;
                        }
                    };
                    let (reader, writer) = stream.into_split();
                    vacant_entry.insert(BufWriter::new(writer));
                    handle_tcp_reader(BufReader::new(reader), addr.ip()).await;
                },
            }
        }

        for (&addr, conn) in sibling_conns.iter_mut() {
            futures.push(async move {
                conn.write_u64(source.len() as u64).await.map_err(|e| (e, addr))?;
                conn.write_all(source.as_bytes()).await.map_err(|e| (e, addr))?;
                conn.write_u64(bytes.len() as u64).await.map_err(|e| (e, addr))?;
                conn.write_all(bytes).await.map_err(|e| (e, addr))?;
                Result::<_, (std::io::Error, IpAddr)>::Ok(())
            });
        }

        while let Some(result) = futures.next().await {
            match result {
                Ok(()) => {}
                Err((e, addr)) => {
                    error!("Failed to send to sibling {}: {}", addr, e);
                    to_remove.push(addr);
                }
            }
        }
    }

    for addr in to_remove {
        sibling_conns.remove(&addr);
    }

    Ok(())
}

pub async fn add_sibling_message_handler_raw(f: impl FnMut(&str, &[u8]) + Send + 'static)
{
    SIBLING_MESSAGE_HANDLERS.lock().await.push(Box::new(f));
}

#[macro_export]
macro_rules! send_to_siblings {
    ($bytes: expr) => {
        send_to_siblings_raw(env!("CARGO_PKG_VERSION").as_bytes(), $bytes)
    };
}

#[macro_export]
macro_rules! add_sibling_message_handler_raw {
    ($f: expr) => {
        add_sibling_message_handler_raw(move |source, bytes| {
            if source != env!("CARGO_PKG_VERSION") {
                return;
            }
            $f(bytes);
        })
    };
}

#[derive(Clone, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "backend_data")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub address: String
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}