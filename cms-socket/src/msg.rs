// -*- coding: utf-8 -*-
//
// Simple CMS
//
// Copyright (C) 2011-2025 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow as ah;
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
pub fn bincode_config() -> impl bincode::config::Config {
    bincode::config::standard()
        .with_limit::<MAX_RX_BUF>()
        .with_little_endian()
        .with_fixed_int_encoding()
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
            bincode::serde::encode_to_vec(
                MsgHdr {
                    magic: 0,
                    payload_len: 0,
                },
                bincode_config()
            )
            .unwrap()
            .len()
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
                use bincode::serde::encode_to_vec;
                use $crate::{MsgHdr, bincode_config};

                let mut payload = encode_to_vec(self, bincode_config())?;
                let mut ret = encode_to_vec(MsgHdr::new($magic, payload.len()), bincode_config())?;
                ret.append(&mut payload);
                Ok(ret)
            }

            fn try_msg_deserialize(buf: &[u8]) -> anyhow::Result<$crate::DeserializeResult<Msg>> {
                use anyhow::Context as _;
                use bincode::serde::borrow_decode_from_slice;
                use $crate::{MSG_HDR_LEN, MsgHdr, bincode_config};

                let hdr_len = MsgHdr::len();
                if buf.len() < hdr_len {
                    Ok($crate::DeserializeResult::Pending(hdr_len - buf.len()))
                } else {
                    let (hdr, size): (MsgHdr, usize) =
                        borrow_decode_from_slice(&buf[0..hdr_len], bincode_config())
                            .context("Deserialize MsgHdr")?;
                    if size != MSG_HDR_LEN {
                        return Err(anyhow::format_err!("Deserialize: Invalid header size."));
                    }
                    if hdr.magic() != $magic {
                        return Err(anyhow::format_err!("Deserialize: Invalid magic code."));
                    }
                    let full_len = hdr_len
                        .checked_add(hdr.payload_len())
                        .context("Msg length overflow")?;
                    if buf.len() < full_len {
                        Ok($crate::DeserializeResult::Pending(full_len - buf.len()))
                    } else {
                        let (msg, size) =
                            borrow_decode_from_slice(&buf[hdr_len..full_len], bincode_config())
                                .context("Deserialize Msg")?;
                        if size != hdr.payload_len() {
                            return Err(anyhow::format_err!("Deserialize: Invalid payload size."));
                        }
                        Ok($crate::DeserializeResult::Ok(msg))
                    }
                }
            }
        }
    };
}

// vim: ts=4 sw=4 expandtab
