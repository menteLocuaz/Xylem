use crate::editor::messages::{ServerCommand, XylemMessage, RpcRequest};
use crate::editor::events::EditorEvent;
use crate::parser::installer::ParserInstaller;
use crate::parser::registry::GrammarSpec;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;

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
        let mut stdin = BufReader::new(stdin());

        while self.running.load(Ordering::SeqCst) {
            let mut header = String::new();
            if stdin.read_line(&mut header).await? == 0 { break; }

            if header.starts_with("Content-Length: ") {
                let len: usize = header["Content-Length: ".len()..].trim().parse()?;

                let mut next_line = String::new();
                stdin.read_line(&mut next_line).await?;

                let mut body = vec![0u8; len];
                tokio::io::AsyncReadExt::read_exact(&mut stdin, &mut body).await?;

                if let Ok(request) = serde_json::from_slice::<RpcRequest>(&body) {
                    self.handle_request(request).await?;
                }
            } else if !header.trim().is_empty() {
                if let Ok(request) = serde_json::from_str::<RpcRequest>(&header) {
                    self.handle_request(request).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn handle_request(&self, request: RpcRequest) -> anyhow::Result<()> {
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
                        println!("Installed to: {:?}", path);
                    }
                });
            }
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}
