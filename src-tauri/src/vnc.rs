//! A local WebSocket server that bridges the frontend to SSH channels: noVNC to a forwarded
//! VNC connection (what websockify does, but in-process), and the terminal to a shell.
//! It listens on 127.0.0.1 only, and every tunnel gets a one-time random token in the URL path.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use russh::Channel;
use russh::client::Msg;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::OnceCell;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};

use crate::ssh::Conn;

/// How long an opened tunnel waits for noVNC to connect.
const PENDING_TTL: Duration = Duration::from_secs(60);

/// What travels over a tunnel.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Bytes both ways (VNC).
    Raw,
    /// A shell with a terminal: bytes both ways, plus `{"cols":..,"rows":..}` text messages
    /// that resize the terminal.
    Terminal,
}

struct Pending {
    conn: Arc<Conn>,
    channel: Channel<Msg>,
    kind: Kind,
    created: Instant,
}

#[derive(Default)]
pub struct Bridge {
    port: OnceCell<u16>,
    pending: Mutex<HashMap<String, Pending>>,
}

impl Bridge {
    /// Registers a forwarded VNC channel and returns the WebSocket URL noVNC should open.
    pub async fn register(self: &Arc<Self>, conn: Arc<Conn>, channel: Channel<Msg>, kind: Kind) -> Result<String> {
        let port = *self
            .port
            .get_or_try_init(|| async {
                let listener = TcpListener::bind("127.0.0.1:0").await?;
                let port = listener.local_addr()?.port();
                let me = self.clone();
                tokio::spawn(async move { me.accept_loop(listener).await });
                anyhow::Ok(port)
            })
            .await?;
        let mut b = [0u8; 16];
        getrandom::fill(&mut b).expect("OS random source");
        let token: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|_, p| p.created.elapsed() < PENDING_TTL);
        pending.insert(token.clone(), Pending { conn, channel, kind, created: Instant::now() });
        Ok(format!("ws://127.0.0.1:{port}/{token}"))
    }

    async fn accept_loop(self: Arc<Self>, listener: TcpListener) {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let me = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = me.serve(stream).await {
                            log::info!("VNC bridge: {e:#}");
                        }
                    });
                }
                Err(e) => {
                    log::warn!("VNC bridge accept: {e}");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }

    async fn serve(&self, stream: TcpStream) -> Result<()> {
        stream.set_nodelay(true).ok();
        let mut taken: Option<Pending> = None;
        let callback = |req: &Request, mut resp: Response| -> Result<Response, ErrorResponse> {
            let token = req.uri().path().trim_start_matches('/');
            let p = self.pending.lock().unwrap().remove(token);
            match p {
                Some(p) if p.created.elapsed() < PENDING_TTL => taken = Some(p),
                _ => {
                    let mut r = ErrorResponse::new(Some("unknown or expired tunnel".into()));
                    *r.status_mut() = StatusCode::NOT_FOUND;
                    return Err(r);
                }
            }
            // noVNC may ask for the "binary" subprotocol; a browser drops the socket if it is not echoed.
            let offers_binary = req
                .headers()
                .get_all("Sec-WebSocket-Protocol")
                .iter()
                .filter_map(|v| v.to_str().ok())
                .any(|v| v.split(',').any(|p| p.trim() == "binary"));
            if offers_binary {
                resp.headers_mut().insert("Sec-WebSocket-Protocol", HeaderValue::from_static("binary"));
            }
            Ok(resp)
        };
        let ws = tokio_tungstenite::accept_hdr_async(stream, callback).await?;
        let Pending { conn, channel, kind, .. } = taken.expect("set by the handshake callback");
        let _keep_ssh_alive = conn;
        if kind == Kind::Terminal {
            return serve_terminal(ws, channel).await;
        }

        let (mut ws_tx, mut ws_rx) = ws.split();
        let (mut rd, mut wr) = tokio::io::split(channel.into_stream());

        let upstream = async {
            while let Some(msg) = ws_rx.next().await {
                match msg? {
                    Message::Binary(b) => wr.write_all(&b).await?,
                    Message::Text(t) => wr.write_all(t.as_bytes()).await?,
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            wr.shutdown().await.ok();
            anyhow::Ok(())
        };
        let downstream = async {
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = rd.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                ws_tx.send(Message::Binary(buf[..n].to_vec().into())).await?;
            }
            ws_tx.send(Message::Close(None)).await.ok();
            anyhow::Ok(())
        };
        tokio::select! {
            r = upstream => r,
            r = downstream => r,
        }
    }
}

#[derive(serde::Deserialize)]
struct Resize {
    cols: u32,
    rows: u32,
}

async fn serve_terminal(ws: tokio_tungstenite::WebSocketStream<TcpStream>, channel: Channel<Msg>) -> Result<()> {
    use russh::ChannelMsg;
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (mut rd, wr) = channel.split();
    let mut input = wr.make_writer();

    let upstream = async {
        while let Some(msg) = ws_rx.next().await {
            match msg? {
                Message::Binary(b) => {
                    input.write_all(&b).await?;
                    input.flush().await?;
                }
                Message::Text(t) => {
                    if let Ok(r) = serde_json::from_str::<Resize>(t.as_str()) {
                        wr.window_change(r.cols.max(1), r.rows.max(1), 0, 0).await.ok();
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        wr.eof().await.ok();
        anyhow::Ok(())
    };
    let downstream = async {
        while let Some(msg) = rd.wait().await {
            match msg {
                ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                    ws_tx.send(Message::Binary(data.to_vec().into())).await?;
                }
                ChannelMsg::Eof | ChannelMsg::Close => break,
                ChannelMsg::ExitStatus { .. } => {}
                _ => {}
            }
        }
        ws_tx.send(Message::Close(None)).await.ok();
        anyhow::Ok(())
    };
    tokio::select! {
        r = upstream => r,
        r = downstream => r,
    }
}
