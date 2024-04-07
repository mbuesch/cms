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

use crate::msg::{DeserializeResult, MsgSerde, MAX_RX_BUF, MSG_HDR_LEN};
use anyhow::{self as ah, format_err as err, Context as _};
use std::{
    io::{Read as _, Write as _},
    os::unix::net::UnixStream,
    path::Path,
};

pub struct CmsSocketConnSync {
    stream: UnixStream,
}

impl CmsSocketConnSync {
    fn new(stream: UnixStream) -> Self {
        Self { stream }
    }

    pub fn connect(path: &Path) -> ah::Result<Self> {
        Ok(Self::new(UnixStream::connect(path)?))
    }

    pub fn recv_msg<F, M>(&mut self, try_deserialize: F) -> ah::Result<Option<M>>
    where
        F: Fn(&[u8]) -> ah::Result<DeserializeResult<M>>,
    {
        let mut rxbuf = vec![0; MSG_HDR_LEN];
        self.stream.read_exact(&mut rxbuf).context("Socket read")?;
        match try_deserialize(&rxbuf)? {
            DeserializeResult::Ok(msg) => {
                return Ok(Some(msg));
            }
            DeserializeResult::Pending(count) => {
                let size = MSG_HDR_LEN.saturating_add(count);
                if size > MAX_RX_BUF {
                    return Err(err!("RX buffer overrun."));
                }
                rxbuf.resize(size, 0);
                self.stream
                    .read_exact(&mut rxbuf[MSG_HDR_LEN..])
                    .context("Socket read")?;
            }
        }
        match try_deserialize(&rxbuf)? {
            DeserializeResult::Ok(msg) => {
                return Ok(Some(msg));
            }
            DeserializeResult::Pending(_) => {
                unreachable!();
            }
        }
    }

    pub fn send_msg<M>(&mut self, msg: &impl MsgSerde<M>) -> ah::Result<()> {
        let txbuf = msg.msg_serialize()?;
        self.stream.write_all(&txbuf).context("Socket write")
    }
}

// vim: ts=4 sw=4 expandtab
