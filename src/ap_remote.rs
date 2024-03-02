use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::ops::{Range, RangeInclusive};
use std::sync::Arc;
use std::time::Duration;
use std::{str, thread};

use anyhow::{anyhow, Context, Error, Result};

use base64::Engine;
use itertools::Itertools;
use parking_lot::Mutex;
use serde::{de, Deserialize, Deserializer, Serializer};
use sha1::{Digest, Sha1};

use crate::core::{self, Core};

mod request;
use request::*;

mod response;
use response::*;

const VERSION: u8 = 1;
const FIRST_PORT: u16 = 43055;
const NUM_PORTS: u16 = 5;

pub fn start(core_handle: core::Handle) {
    thread::spawn(move || {
        if let Err(err) = try_start(core_handle) {
            eprintln!("ap remote interface stopped with error: {err:#?}");
        }
    });
}

fn try_start(core_handle: core::Handle) -> Result<()> {
    let socket = bind_socket().context("failed to create socket")?;

    loop {
        let stream = match socket.accept() {
            Ok((stream, _sockaddr)) => stream,
            Err(err) => {
                eprintln!("Accepting ap remote client failed: {err:?}");
                continue;
            }
        };

        let core_handle = core_handle.clone();

        thread::spawn(move || handle_client(stream, core_handle));
    }
}

fn handle_client(stream: TcpStream, core_handle: core::Handle) {
    if let Err(err) = try_handle_client(stream, core_handle) {
        eprintln!("Error handling ap remote client: {err:?}");
    }
}

fn try_handle_client(stream: TcpStream, core_handle: core::Handle) -> Result<()> {
    // TODO: move to tokio for proper message receive timeouts
    stream.set_read_timeout(Some(Duration::from_secs(60)))?;
    stream.set_write_timeout(Some(Duration::from_secs(60)))?;

    let mut stream = BufReader::new(stream);

    loop {
        let requests = receive_requests(&mut stream).context("failed to receive requests")?;
        let Some(requests) = requests else {
            eprintln!("ap remote client disconnected");
            return Ok(());
        };

        let responses;

        (responses, stream) = core_handle
            .run(move |core| {
                let responses = handle_requests(requests, core, &mut stream);
                (responses, stream)
            })
            .context("failed to run in core")?;

        let responses = responses.context("failed to handle requests")?;

        let Some(responses) = responses else {
            eprintln!("ap remote client disconnected");
            return Ok(());
        };

        send_responses(&mut stream, responses).context("failed to send responses")?;
    }
}

fn receive_requests(stream: &mut BufReader<TcpStream>) -> Result<Option<Vec<Request>>> {
    let mut requests = String::new();
    let num_read = stream
        .read_line(&mut requests)
        .context("failed to receive line")?;

    if num_read == 0 {
        return Ok(None);
    }

    let requests = parse_requests(&requests).context("failed to parse request")?;

    Ok(Some(requests))
}

fn parse_requests(request: &str) -> Result<Vec<Request>> {
    if request.trim().eq_ignore_ascii_case("VERSION") {
        return Ok(vec![Request::Version]);
    }

    let request = serde_json::from_str::<Vec<Request>>(request)?;

    Ok(request)
}

fn handle_requests(
    mut requests: Vec<Request>,
    core: &mut Core,
    stream: &mut BufReader<TcpStream>,
) -> Result<Option<Vec<Response>>> {
    let mut is_locked = false;
    let mut failed_guard: Option<Response> = None;

    let responses = loop {
        let mut responses = Vec::with_capacity(requests.len());

        for request in requests {
            if let Some(failed_guard) = &failed_guard {
                responses.push(failed_guard.clone())
            }

            let response = handle_request(request, core)?;

            match response {
                Response::Locked => is_locked = true,
                Response::Unlocked => is_locked = false,
                Response::GuardResponse { .. } => {
                    failed_guard = Some(response);
                    continue;
                }
                _ => {}
            }

            responses.push(response);
        }

        if !is_locked {
            break responses;
        }

        send_responses(stream, responses).context("failed to send responses")?;

        requests = match receive_requests(stream).context("failed to receive requests")? {
            Some(requests) => requests,
            None => return Ok(None),
        }
    };

    Ok(Some(responses))
}

fn handle_request(request: Request, core: &mut Core) -> Result<Response> {
    Ok(match request {
        Request::Version => Response::Version,
        Request::Ping => {
            eprintln!("Received ping from ap remote client");
            Response::Pong
        }
        Request::System => Response::SystemResponse {
            // TODO: report correct system
            value: "GBA".into(),
        },
        Request::PreferredCores => Response::Error {
            err: format!("TODO: unimplemented command: PreferredCores"),
        },
        Request::Hash => Response::HashResponse {
            value: core.get_sha1_romhash(),
        },
        Request::Guard {
            address,
            expected_data,
            domain,
        } => match &*domain {
            "ROM" => core.rom(|rom| {
                let start = address.min(rom.len());
                let end = address.saturating_add(expected_data.len()).min(rom.len());
                let data = &rom[start..end];
                let is_match = data == expected_data;

                if expected_data.len() != data.len() {
                    eprintln!("WARNING: incomplete read");
                }

                Response::GuardResponse {
                    value: is_match,
                    address,
                }
            }),
            "System Bus" => {
                let max_len = expected_data.len();
                let data = core.get_memory(address, max_len);
                let is_match = data == expected_data;

                if expected_data.len() != data.len() {
                    eprintln!("WARNING: incomplete read");
                }

                Response::GuardResponse {
                    value: is_match,
                    address,
                }
            }
            _ => Response::Error {
                err: format!("Unknown memory domain: {domain:?}"),
            },
        },
        Request::Lock => Response::Error {
            err: format!("TODO: unimplemented command: Lock"),
        },
        Request::Unlock => Response::Error {
            err: format!("TODO: unimplemented command: Unlock"),
        },
        Request::Read {
            address,
            size,
            domain,
        } => match &*domain {
            "ROM" => core.rom(|rom| {
                let start = address.min(rom.len());
                let end = address.saturating_add(size).min(rom.len());
                let data = rom[start..end].to_vec();

                if size != data.len() {
                    eprintln!("WARNING: incomplete read");
                }

                Response::ReadResponse { value: data }
            }),
            "System Bus" => {
                let max_len = size;
                let data = core.get_memory(address, max_len);

                if size != data.len() {
                    eprintln!("WARNING: incomplete read");
                }

                Response::ReadResponse { value: data }
            }
            _ => Response::Error {
                err: format!("Unknown memory domain: {domain:?}"),
            },
        },
        Request::Write { address, value } => {
            let bytes_written = core.write_memory(address, &value);

            if value.len() != bytes_written {
                eprintln!("WARNING: incomplete write!");
            }

            Response::WriteResponse
        }
        Request::DisplayMessage { message } => Response::Error {
            err: format!("TODO: unimplemented command: DisplayMessage"),
        },
        Request::SetMessageInterval { value } => Response::Error {
            err: format!("TODO: unimplemented command: SetMessageInterval"),
        },
    })
}

fn send_responses(stream: &mut BufReader<TcpStream>, responses: Vec<Response>) -> Result<()> {
    if let Some(Response::Version) = responses.first() {
        let version = format!("{VERSION}\n");

        stream.get_mut().write_all(version.as_bytes())?;
        stream.get_mut().flush()?;

        return Ok(());
    }

    let mut responses = serde_json::to_string(&responses)?;
    responses.push('\n');

    stream.get_mut().write_all(responses.as_bytes())?;
    stream.get_mut().flush()?;

    Ok(())
}

fn bind_socket() -> Result<TcpListener, Error> {
    let mut errors = None::<Error>;
    let port_range = FIRST_PORT..FIRST_PORT + 5;

    for port in port_range {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .with_context(|| anyhow!("failed to listen on port {port}"));

        match listener {
            Ok(listener) => return Ok(listener),
            Err(err) => {
                errors = match errors.take() {
                    Some(errors) => Some(err.context(errors)),
                    None => Some(err),
                }
            }
        }
    }

    let err = errors
        .map(|errors| errors.context("no port found to listen on"))
        .unwrap_or_else(|| anyhow!("empty range of ports"));

    Err(err)
}

fn deserialize_base64<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<u8>, D::Error> {
    let data = <&str>::deserialize(de)?;
    let data = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(de::Error::custom)?;

    Ok(data)
}

fn serialize_base64<S: Serializer>(data: &[u8], ser: S) -> Result<S::Ok, S::Error> {
    let data = base64::engine::general_purpose::STANDARD.encode(data);

    ser.serialize_str(&data)
}
