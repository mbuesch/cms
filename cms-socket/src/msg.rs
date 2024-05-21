// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2024 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow as ah;
use bincode::Options as _;
use serde::{Deserialize, Serialize};

pub const MSG_HDR_LEN: usize = 8;
pub const MAX_RX_BUF: usize = 1024 * 1024 * 64;

#[derive(Clone, Debug)]
pub enum DeserializeResult<M> {
    Ok(M),
    Pending(usize),
}

pub trait MsgSerde<M> {
    fn msg_serialize(&self) -> ah::Result<Vec<u8>>;
    fn try_msg_deserialize(buf: &[u8]) -> ah::Result<DeserializeResult<M>>;
}

#[inline]
pub fn bincode_config() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_RX_BUF.try_into().unwrap())
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
