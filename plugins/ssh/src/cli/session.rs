use crate::{server_by_name, test_server};
use anyhow::{Context, Result};
use dm_plugin_sdk::Context as PluginContext;
use russh::ChannelMsg;
use std::io::{IsTerminal, Read};
use tokio::io::AsyncWriteExt;

pub(super) fn test(context: &PluginContext, name: &str) -> Result<()> {
    test_server(context, &server_by_name(context, name)?)?;
    println!("SSH 连接 {name} 测试成功");
    Ok(())
}
struct RawTerminal(bool);
impl Drop for RawTerminal {
    fn drop(&mut self) {
        if self.0 {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}
pub(super) fn connect(context: &PluginContext, name: &str) -> Result<i32> {
    let server = server_by_name(context, name)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let session = crate::domain::ssh_command::authenticate(context, &server).await?;
        let mut channel = session.channel_open_session().await?;
        let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        if interactive {
            let (width, height) = crossterm::terminal::size()?;
            channel.request_pty(true, &std::env::var("TERM").unwrap_or("xterm".into()),
                width.into(), height.into(), 0, 0, &[]).await?;
            crossterm::terminal::enable_raw_mode()?;
        }
        let _terminal = RawTerminal(interactive);
        channel.request_shell(true).await?;
        // A detached reader avoids Tokio's uncancellable stdin task keeping
        // runtime shutdown blocked after the remote side exits.
        let (sender, mut input) = tokio::sync::mpsc::channel(16);
        std::thread::spawn(move || {
            let mut stdin = std::io::stdin();
            loop {
                let mut buffer = [0; 4096];
                let bytes = stdin.read(&mut buffer).map(|n| buffer[..n].to_vec());
                let done = !bytes.as_ref().is_ok_and(|value| !value.is_empty());
                if sender.blocking_send(bytes).is_err() || done { break; }
            }
        });
        let mut stdout = tokio::io::stdout();
        let mut stderr = tokio::io::stderr();
        let mut eof = false;
        let mut code = None;
        loop {
            tokio::select! {
                bytes = input.recv(), if !eof => {
                    let bytes = bytes.transpose()?.unwrap_or_default();
                    if bytes.is_empty() { eof = true; channel.eof().await?; }
                    else { channel.data(bytes.as_slice()).await?; }
                }
                message = channel.wait() => match message {
                    Some(ChannelMsg::Data { data }) => { stdout.write_all(&data).await?; stdout.flush().await?; }
                    Some(ChannelMsg::ExtendedData { data, .. }) => { stderr.write_all(&data).await?; stderr.flush().await?; }
                    Some(ChannelMsg::ExitStatus { exit_status }) => { code = Some(exit_status as i32); }
                    Some(ChannelMsg::ExitSignal { .. }) => { code = Some(1); }
                    Some(ChannelMsg::Failure) => anyhow::bail!("SSH shell request failed"),
                    Some(ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
        }
        code.context("SSH session closed without an exit status")
    })
}
