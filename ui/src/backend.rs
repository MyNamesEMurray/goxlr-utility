use crate::app::Msg;
use crate::{APP, AppWindow};
use anyhow::{Result, anyhow};
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse, DaemonStatus};
use interprocess::local_socket::tokio::prelude::LocalSocketStream;
use interprocess::local_socket::traits::tokio::Stream;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, ToFsName, ToNsName};
use slint::Weak;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

const SOCKET_PATH: &str = "/tmp/goxlr.socket";
const NAMED_PIPE: &str = "@goxlr.socket";

pub async fn connect() -> Result<LocalSocketStream> {
    let name = if cfg!(windows) {
        NAMED_PIPE.to_ns_name::<GenericNamespaced>()
    } else {
        SOCKET_PATH.to_fs_name::<GenericFilePath>()
    };
    Ok(LocalSocketStream::connect(name?).await?)
}

pub type Sock = Socket<DaemonResponse, DaemonRequest>;

pub async fn request(sock: &mut Sock, req: DaemonRequest) -> Result<DaemonResponse> {
    sock.send(req).await?;
    sock.read()
        .await
        .ok_or_else(|| anyhow!("daemon closed the connection"))?
        .map_err(Into::into)
}

fn ui_call(f: impl FnOnce(&mut crate::app::App) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        APP.with(|a| {
            if let Some(app) = a.borrow_mut().as_mut() {
                f(app);
            }
        })
    });
}

pub fn spawn(rx: UnboundedReceiver<Msg>, _ui: Weak<AppWindow>) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(run(rx));
    });
}

async fn run(mut rx: UnboundedReceiver<Msg>) {
    let mut last = String::new();
    let mut mic: Option<String> = None;
    loop {
        let stream = match connect().await {
            Ok(s) => s,
            Err(_) => {
                if last != "offline" {
                    last = "offline".into();
                    ui_call(|a| a.on_status(None));
                }
                while let Ok(m) = rx.try_recv() {
                    if let Msg::Mic(m) = m {
                        mic = m;
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let mut sock: Sock = Socket::new(stream);
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    match request(&mut sock, DaemonRequest::GetStatus).await {
                        Ok(DaemonResponse::Status(st)) => publish(&mut last, st),
                        Ok(_) => {}
                        Err(_) => break,
                    }
                    if let Some(serial) = &mic {
                        match request(&mut sock, DaemonRequest::GetMicLevel(serial.clone())).await {
                            Ok(DaemonResponse::MicLevel(l)) => ui_call(move |a| a.set_mic_level(l as f32)),
                            Ok(_) => {}
                            Err(_) => break,
                        }
                    }
                }
                msg = rx.recv() => {
                    let Some(msg) = msg else { return };
                    let req = match msg {
                        Msg::Cmd(serial, c) => DaemonRequest::Command(serial, c),
                        Msg::Daemon(c) => DaemonRequest::Daemon(c),
                        Msg::Mic(m) => { mic = m; continue; }
                    };
                    match request(&mut sock, req).await {
                        Ok(DaemonResponse::Error(e)) => ui_call(move |a| a.set_error(&e)),
                        Ok(_) => ui_call(|a| a.set_error("")),
                        Err(_) => break,
                    }
                }
            }
        }
        last.clear();
    }
}

fn publish(last: &mut String, st: DaemonStatus) {
    let json = serde_json::to_string(&st).unwrap_or_default();
    if json != *last {
        *last = json;
        ui_call(move |a| a.on_status(Some(st)));
    }
}
