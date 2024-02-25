use std::fmt::Write;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::{str, thread};

use anyhow::{anyhow, Context, Result};

use itertools::Itertools;

use crate::core;

pub fn start(core_handle: core::Handle) {
    thread::spawn(move || {
        if let Err(err) = try_start(core_handle) {
            eprintln!("remote interface stopped with error: {err:#?}");
        }
    });
}

fn try_start(core_handle: core::Handle) -> Result<()> {
    let socket =
        UdpSocket::bind((Ipv4Addr::LOCALHOST, 55355)).context("failed to create socket")?;
    let msg = &mut [0; 2048];

    loop {
        let (len, sockaddr) = socket
            .recv_from(msg)
            .context("remote: failed to recv message")?;
        let msg = &msg[..len];

        if let Err(err) = handle_message(&core_handle, &socket, sockaddr, msg) {
            eprintln!("remote: failed to handle message: {err:?}")
        }
    }
}

fn handle_message(
    core_handle: &core::Handle,
    socket: &UdpSocket,
    reply_addr: SocketAddr,
    msg: &[u8],
) -> Result<()> {
    let msg = str::from_utf8(msg)
        // .map_err(|err| anyhow!("{err}"))
        .context("message contained invalid utf-8")?;
    let mut parts = msg.split_whitespace();
    let command = parts.next().context("received message without command")?;
    let context = CommandContext {
        core_handle,
        socket,
        reply_addr,
        args: &mut parts,
    };

    context.handle_command(command)?;

    Ok(())
}

struct CommandContext<'a, I> {
    core_handle: &'a core::Handle,
    socket: &'a UdpSocket,
    reply_addr: SocketAddr,
    args: &'a mut I,
}

impl<'a, I> CommandContext<'a, I>
where
    I: Iterator<Item = &'a str>,
{
    fn reply(self, message: impl AsRef<[u8]>) -> Result<()> {
        self.socket
            .send_to(message.as_ref(), self.reply_addr)
            .context("failed to send reply")?;

        Ok(())
    }

    fn handle_command(self, command: &str) -> Result<()> {
        match command {
            "VERSION" => self
                .handle_version()
                .context("failed to handle VERSION command")?,
            "GET_STATUS" => self
                .handle_get_status()
                .context("failed to handle GET_STATUS command")?,
            "READ_CORE_MEMORY" => self
                .handle_read_core_memory()
                .context("failed to handle READ_CORE_MEMORY command")?,
            "WRITE_CORE_MEMORY" => self
                .handle_write_core_memory()
                .context("failed to handle WRITE_CORE_MEMORY command")?,
            _ => {
                eprintln!("unknown command `{command:?}`");
            }
        }

        Ok(())
    }

    fn handle_version(self) -> Result<()> {
        self.reply(b"1.14.0\n")
    }

    fn handle_get_status(self) -> Result<()> {
        let system_info = self
            .core_handle
            .run(|core| core.get_system_info().to_owned())?;

        let system_id = system_info.system_id.unwrap_or(&system_info.library_name);

        self.reply(format!(
            "GET_STATUS PLAYING {system_id},TODO_romname,TODO_hash\n"
        ))
    }

    fn handle_read_core_memory(self) -> Result<()> {
        let (address_str, len) = self.args.next_tuple().context("invalid number of args")?;
        let address = address_str.strip_prefix("0x").unwrap_or(address_str);
        let address = usize::from_str_radix(address, 16).context("invalid address format")?;
        let len = len.parse::<usize>().context("invalid len format")?;
        let mut msg =
            String::with_capacity("READ_CORE_MEMORY ".len() + address_str.len() + len * 3 + 1);

        let mem = self
            .core_handle
            .run(move |core| core.get_memory(address, len).to_vec())?;

        msg.push_str("READ_CORE_MEMORY ");
        msg.push_str(address_str);

        for byte in mem {
            write!(msg, " {byte:02X}").ok();
        }

        msg.push('\n');

        self.reply(msg)
    }

    fn handle_write_core_memory(self) -> Result<()> {
        let address_str = self.args.next().context("invalid number of args")?;
        let address = address_str.strip_prefix("0x").unwrap_or(address_str);
        let address = usize::from_str_radix(address, 16).context("invalid address format")?;
        let bytes = self
            .args
            .map(|byte| {
                let byte = byte.strip_prefix("0x").unwrap_or(byte);
                u8::from_str_radix(byte, 16).map_err(|err| anyhow!("invalid byte `{byte}` {err:?}"))
            })
            .collect::<Result<Vec<_>>>()
            .context("invalid byte format")?;

        let bytes_written = self
            .core_handle
            .run(move |core| core.write_memory(address, &bytes))?;

        self.reply(format!("WRITE_CORE_MEMORY {address_str} {bytes_written}\n"))
    }
}
