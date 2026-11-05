//! Maintainer interoperability probe; these unrelated types exercise generic routing.
use cev_onboarding::{shared::transport::Transport, Result};
use prost::Message;
use zenoh::Wait;
#[derive(Clone, PartialEq, Message)]
struct Note {
    #[prost(string, tag = "1")]
    title: String,
    #[prost(uint64, tag = "2")]
    nonce: u64,
}
#[derive(Clone, PartialEq, Message)]
struct Reply {
    #[prost(bool, tag = "1")]
    accepted: bool,
}
fn main() -> Result<()> {
    let run = std::env::args()
        .nth(1)
        .ok_or_else(|| cev_onboarding::error("Specify run id"))?;
    let io = Transport::connect(&run)?;
    let sub = io
        .session
        .declare_subscriber(format!("test/{run}/note"))
        .with(zenoh::handlers::RingChannel::new(1))
        .wait()?;
    println!("READY");
    let sample = sub
        .recv_timeout(std::time::Duration::from_secs(10))?
        .ok_or_else(|| cev_onboarding::error("Browser note timed out"))?;
    let note = Note::decode(sample.payload().to_bytes().as_ref())?;
    if note.title != "browser" || note.nonce != u64::MAX {
        return Err(cev_onboarding::error("Cross-language note changed"));
    }
    io.session
        .put(
            format!("test/{run}/reply"),
            Reply { accepted: true }.encode_to_vec(),
        )
        .wait()?;
    // Allow the reliable transport to deliver before closing the session.
    std::thread::sleep(std::time::Duration::from_millis(100));
    println!("Rust decoded exact u64 and published reply");
    Ok(())
}
