//! An actual SSH protocol peer, running only on loopback; no system tools.
use russh::{
    ChannelId,
    keys::{Algorithm, PrivateKey, ssh_key::PublicKey},
    server::{self, Server as _, Session},
};
use std::{sync::Arc, time::Duration};

pub struct Fixture {
    pub port: u16,
    pub key: PrivateKey,
    runtime: tokio::runtime::Runtime,
}
#[derive(Clone)]
struct Peer(PublicKey);
impl server::Server for Peer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}
impl server::Handler for Peer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "user" && password == "fixture-password" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn auth_publickey(
        &mut self,
        user: &str,
        key: &PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "user" && key == &self.0 {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        _: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.data(channel, data.to_vec())?;
        Ok(())
    }
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.exit_status_request(channel, 7)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}
impl Fixture {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
        let public = key.public_key().clone();
        let config = Arc::new(server::Config {
            keys: vec![PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()],
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let port = runtime.block_on(async {
            let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = socket.local_addr().unwrap().port();
            runtime.spawn(async move {
                Peer(public).run_on_socket(config, &socket).await.unwrap();
            });
            port
        });
        Self { port, key, runtime }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = &self.runtime;
    }
}
