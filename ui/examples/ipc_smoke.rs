use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse, GoXLRCommand};
use goxlr_types::ChannelName;
use goxlr_ui::backend::{Sock, connect, request};

#[tokio::main]
async fn main() {
    let mut sock: Sock = Socket::new(connect().await.expect("connect"));
    match request(&mut sock, DaemonRequest::GetStatus).await.expect("status") {
        DaemonResponse::Status(s) => println!("status ok, mixers: {:?}", s.mixers.keys().collect::<Vec<_>>()),
        other => panic!("unexpected {other:?}"),
    }
    match request(&mut sock, DaemonRequest::GetMicLevel("S12345".into())).await.expect("mic") {
        DaemonResponse::MicLevel(l) => println!("mic level {l}"),
        other => panic!("unexpected {other:?}"),
    }
    match request(&mut sock, DaemonRequest::Command("S12345".into(), GoXLRCommand::SetVolume(ChannelName::Mic, 200))).await.expect("cmd") {
        DaemonResponse::Ok => println!("command ok"),
        other => panic!("unexpected {other:?}"),
    }
}
