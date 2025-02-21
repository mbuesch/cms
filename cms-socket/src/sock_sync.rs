// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::msg::{DeserializeResult, MAX_RX_BUF, MSG_HDR_LEN, MsgSerde};
use anyhow::{self as ah, Context as _, format_err as err};
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
        if let DeserializeResult::Ok(msg) = try_deserialize(&rxbuf)? {
            return Ok(Some(msg));
        }
        unreachable!();
    }

    pub fn send_msg<M>(&mut self, msg: &impl MsgSerde<M>) -> ah::Result<()> {
        let txbuf = msg.msg_serialize()?;
        self.stream.write_all(&txbuf).context("Socket write")
    }
}

// vim: ts=4 sw=4 expandtab
