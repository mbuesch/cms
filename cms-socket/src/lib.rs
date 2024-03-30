// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

#![forbid(unsafe_code)]

use anyhow::{self as ah, format_err as err, Context as _};
use bincode::Options as _;
use cms_systemd::{have_systemd, systemd_notify_ready, unix_from_systemd};
use libc::{S_IFMT, S_IFSOCK};
use serde::{Deserialize, Serialize};
use std::{
    fs::{metadata, remove_file},
    io::ErrorKind,
    os::unix::{fs::MetadataExt as _, net::UnixListener as StdUnixListener},
    path::Path,
};
use tokio::net::{unix::SocketAddr, UnixListener, UnixStream};

const MAX_RX_BUF: usize = 1024 * 1024 * 16;

#[derive(Clone, Debug)]
pub enum DeserializeResult<M> {
    Ok(M),
    Pending(usize),
}

pub trait MsgSerde<M> {
    fn msg_serialize(&self) -> ah::Result<Vec<u8>>;
    fn try_msg_deserialize(buf: &[u8]) -> ah::Result<DeserializeResult<M>>;
}

pub struct CmsSocket {
    sock: UnixListener,
}

impl CmsSocket {
    /// Create a new [CmsSocket] with the specified path.
    fn new(sock_path: &Path) -> ah::Result<Self> {
        if let Ok(meta) = metadata(sock_path) {
            if meta.mode() & S_IFMT == S_IFSOCK {
                remove_file(sock_path).context("Remove existing socket")?;
            }
        }
        let sock = UnixListener::bind(sock_path).context("Bind socket")?;
        Ok(Self::from_listener(sock))
    }

    /// Create a new [CmsSocket] instance from the given [tokio] socket.
    fn from_listener(sock: UnixListener) -> Self {
        Self { sock }
    }

    /// Create a new [CmsSocket] instance from the given [std] socket.
    fn from_std_listener(sock: StdUnixListener) -> ah::Result<Self> {
        sock.set_nonblocking(true)
            .context("Set socket non-blocking")?;
        Ok(Self::from_listener(UnixListener::from_std(sock)?))
    }

    /// Create a new [CmsSocket] from Systemd environment
    /// or from the specified path, if there is no Systemd.
    pub fn from_systemd_or_path(no_systemd: bool, sock_path: &Path) -> ah::Result<Self> {
        if !no_systemd && have_systemd() {
            println!("Using socket from systemd.");
            let sock = Self::from_std_listener(unix_from_systemd(true)?)?;
            systemd_notify_ready(true)?;
            Ok(sock)
        } else {
            println!("Creating socket {sock_path:?}.");
            Self::new(sock_path)
        }
    }

    pub async fn accept(&mut self) -> ah::Result<CmsSocketConn> {
        let (stream, _addr) = self.sock.accept().await?;
        Ok(CmsSocketConn::new(stream))
    }
}

pub struct CmsSocketConn {
    stream: UnixStream,
}

impl CmsSocketConn {
    fn new(stream: UnixStream) -> Self {
        Self { stream }
    }

    pub async fn connect(path: &Path) -> ah::Result<Self> {
        Ok(Self::new(UnixStream::connect(path).await?))
    }

    pub async fn recv_msg<F, M>(&mut self, try_deserialize: F) -> ah::Result<Option<M>>
    where
        F: Fn(&[u8]) -> ah::Result<DeserializeResult<M>>,
    {
        const SIZE_STEP: usize = 4096;
        let mut rxbuf = vec![0; SIZE_STEP];
        let mut rxcount = 0;
        loop {
            self.stream
                .readable()
                .await
                .context("Socket polling (rx)")?;

            match self.stream.try_read(&mut rxbuf[rxcount..]) {
                Ok(n) => {
                    if n == 0 {
                        return Ok(None);
                    }
                    rxcount += n;
                    if let DeserializeResult::Ok(msg) = try_deserialize(&rxbuf[..rxcount])? {
                        return Ok(Some(msg));
                    }
                    if rxcount >= rxbuf.len() {
                        if rxbuf.len() >= MAX_RX_BUF {
                            return Err(err!("RX buffer overrun."));
                        }
                        rxbuf.resize(rxbuf.len() + SIZE_STEP, 0);
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => (),
                Err(e) => {
                    return Err(err!("Socket read: {e}"));
                }
            }
        }
    }

    pub async fn send_msg<M>(&mut self, msg: &impl MsgSerde<M>) -> ah::Result<()> {
        let txbuf = msg.msg_serialize()?;
        let mut txcount = 0;
        loop {
            self.stream
                .writable()
                .await
                .context("Socket polling (tx)")?;

            match self.stream.try_write(&txbuf[txcount..]) {
                Ok(n) => {
                    txcount += n;
                    if txcount >= txbuf.len() {
                        return Ok(());
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => (),
                Err(e) => {
                    return Err(err!("Socket write: {e}"));
                }
            }
        }
    }
}

const MSG_HDR_LEN: usize = 8;
const SERDE_LIMIT: u64 = 1024 * 1024;

#[inline]
pub fn bincode_config() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_limit(SERDE_LIMIT)
        .with_native_endian()
        .with_fixint_encoding()
        .reject_trailing_bytes()
}

/// Generic message header.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MsgHdr {
    magic: u32,
    payload_len: u32,
}

impl MsgHdr {
    #[inline]
    pub fn new(magic: u32, payload_len: usize) -> Self {
        Self {
            magic,
            payload_len: payload_len
                .try_into()
                .expect("MsgHdr: Payload length too long"),
        }
    }

    #[inline]
    pub fn magic(&self) -> u32 {
        self.magic
    }

    #[inline]
    pub fn len() -> usize {
        debug_assert_eq!(
            MSG_HDR_LEN,
            bincode_config()
                .serialized_size(&MsgHdr {
                    magic: 0,
                    payload_len: 0,
                })
                .unwrap()
                .try_into()
                .unwrap()
        );
        MSG_HDR_LEN
    }

    #[inline]
    pub fn payload_len(&self) -> usize {
        self.payload_len.try_into().unwrap()
    }
}

#[macro_export]
macro_rules! impl_msg_serde {
    ($struct:ty, $magic:literal) => {
        impl $crate::MsgSerde<$struct> for $struct {
            fn msg_serialize(&self) -> anyhow::Result<Vec<u8>> {
                use anyhow::Context as _;
                use bincode::Options as _;
                use $crate::{bincode_config, MsgHdr};

                let mut payload = bincode_config().serialize(self)?;
                let mut ret = bincode_config().serialize(&MsgHdr::new($magic, payload.len()))?;
                ret.append(&mut payload);
                Ok(ret)
            }

            fn try_msg_deserialize(buf: &[u8]) -> anyhow::Result<$crate::DeserializeResult<Msg>> {
                use anyhow::Context as _;
                use bincode::Options as _;
                use $crate::{bincode_config, MsgHdr};

                let hdr_len = MsgHdr::len();
                if buf.len() < hdr_len {
                    Ok($crate::DeserializeResult::Pending(hdr_len - buf.len()))
                } else {
                    let hdr: MsgHdr = bincode_config()
                        .deserialize(&buf[0..hdr_len])
                        .context("Deserialize MsgHdr")?;
                    if hdr.magic() != $magic {
                        return Err(anyhow::format_err!("Deserialize: Invalid magic code."));
                    }
                    let full_len = hdr_len
                        .checked_add(hdr.payload_len())
                        .context("Msg length overflow")?;
                    if buf.len() < full_len {
                        Ok($crate::DeserializeResult::Pending(full_len - buf.len()))
                    } else {
                        let msg = bincode_config()
                            .deserialize(&buf[hdr_len..full_len])
                            .context("Deserialize Msg")?;
                        Ok($crate::DeserializeResult::Ok(msg))
                    }
                }
            }
        }
    };
}

// vim: ts=4 sw=4 expandtab
