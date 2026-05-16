use crate::editor::messages::{MsgpackRpcIn, ServerCommand, XylemMessage, RpcRequest};
use crate::editor::events::EditorEvent;
use crate::parser::installer::ParserInstaller;
use crate::parser::registry::GrammarSpec;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, stdin};
use tokio::sync::mpsc;

pub enum DecodedMessage {
    Request { msgid: u64, request: RpcRequest },
    Response { msgid: u64 },
    Notification { request: RpcRequest },
}

pub struct XylemServer {
    tx: mpsc::Sender<ServerCommand>,
    running: Arc<AtomicBool>,
}

impl XylemServer {
    pub fn new(tx: mpsc::Sender<ServerCommand>) -> Self {
        Self {
            tx,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub async fn process_stdin(&self) -> anyhow::Result<()> {
        let mut stdin = stdin();
        let mut buf = Vec::new();
        let mut temp = [0u8; 512];

        while self.running.load(Ordering::SeqCst) {
            let n = stdin.read(&mut temp).await?;
            if n == 0 { break; }
            buf.extend_from_slice(&temp[..n]);

            loop {
                match try_decode_rpc_message(&buf) {
                    Ok(Some((consumed, DecodedMessage::Request { msgid, request }))) => {
                        buf.drain(..consumed);
                        self.handle_request(msgid, request).await?;
                    }
                    Ok(Some((consumed, DecodedMessage::Notification { request }))) => {
                        buf.drain(..consumed);
                        self.handle_request(0, request).await?;
                    }
                    Ok(Some((consumed, DecodedMessage::Response { .. }))) => {
                        buf.drain(..consumed);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        eprintln!("[xylem] msgpack decode error: {}", e);
                        buf.clear();
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn handle_request(&self, msgid: u64, request: RpcRequest) -> anyhow::Result<()> {
        match request {
            RpcRequest::Change { buffer_id, start_byte, old_end_byte, new_text } => {
                let event = EditorEvent::Change {
                    buffer_id,
                    start_byte,
                    end_byte: old_end_byte,
                    text: new_text,
                };
                let _ = self.tx.send(ServerCommand::UpdateStateWithReply {
                    event,
                    buffer_id,
                }).await;
            }
            RpcRequest::Attach { buffer_id } => {
                let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::Attach { buffer_id })).await;
            }
            RpcRequest::Detach { buffer_id } => {
                let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::Detach { buffer_id })).await;
            }
            RpcRequest::Parse { buffer_id } => {
                let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::Parse { buffer_id })).await;
            }
            RpcRequest::Install { name, repo, revision, queries } => {
                let spec = GrammarSpec { name, repo, revision, queries };
                tokio::spawn(async move {
                    if let Ok(path) = ParserInstaller::install(spec).await {
                        eprintln!("[xylem] Installed to: {:?}", path);
                    }
                });
            }
            RpcRequest::SyncAll => {
                let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::SyncAll)).await;
            }
            RpcRequest::SyncOne { name } => {
                let _ = self.tx.send(ServerCommand::UpdateState(XylemMessage::SyncOne { name })).await;
            }
            RpcRequest::Info => {
                let _ = self.tx.send(ServerCommand::Info { msgid }).await;
            }
            RpcRequest::GetGrammars => {
                let _ = self.tx.send(ServerCommand::GetGrammars { msgid }).await;
            }
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

fn try_decode_rpc_message(buf: &[u8]) -> anyhow::Result<Option<(usize, DecodedMessage)>> {
    let slice = &mut &buf[..];
    match rmpv::decode::read_value(slice) {
        Ok(value) => {
            let consumed = buf.len() - slice.len();
            let msg = MsgpackRpcIn::from_value(value)?;
            match msg {
                MsgpackRpcIn::Request { msgid, .. } => {
                    let request = msg.into_rpc_request()?;
                    Ok(Some((consumed, DecodedMessage::Request { msgid, request })))
                }
                MsgpackRpcIn::Notification { .. } => {
                    let request = msg.into_rpc_request()?;
                    Ok(Some((consumed, DecodedMessage::Notification { request })))
                }
                MsgpackRpcIn::Response { msgid, .. } => {
                    Ok(Some((consumed, DecodedMessage::Response { msgid })))
                }
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                Ok(None)
            } else {
                Err(anyhow::anyhow!("msgpack decode error: {}", e))
            }
        }
    }
}
