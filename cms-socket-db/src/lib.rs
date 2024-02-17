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

use anyhow::{self as ah, Context as _};
use bincode::Options;
use cms_ident::Ident;
use cms_socket::{DeserializeResult, MsgSerde};
use serde::{Deserialize, Serialize};

pub const SOCK_FILE: &str = "cms-fsd.sock";
const SERDE_LIMIT: u64 = 1024 * 1024;
const MAGIC: u32 = 0x8F5755D6;
const MSG_HDR_LEN: usize = 8;

#[inline]
fn bincode_config() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(SERDE_LIMIT)
        .with_native_endian()
        .with_fixint_encoding()
        .reject_trailing_bytes()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct MsgHdr {
    magic: u32,
    payload_len: u32,
}

impl MsgHdr {
    #[inline]
    fn len() -> usize {
        debug_assert_eq!(
            MSG_HDR_LEN,
            bincode_config()
                .serialized_size(&MsgHdr {
                    magic: MAGIC,
                    payload_len: u32::MAX,
                })
                .unwrap()
                .try_into()
                .unwrap()
        );
        MSG_HDR_LEN
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Msg {
    // Getters
    GetPage {
        path: Ident,
        get_title: bool,
        get_data: bool,
        get_stamp: bool,
        get_prio: bool,
        get_redirect: bool,
        get_nav_stop: bool,
        get_nav_label: bool,
    },
    GetHeaders {
        path: Ident,
    },
    GetSubPages {
        path: Ident,
    },
    GetMacro {
        parent: Ident,
        name: Ident,
    },
    GetString {
        name: Ident,
    },

    // Values
    Page {
        title: Option<Vec<u8>>,
        data: Option<Vec<u8>>,
        stamp: Option<u64>,
        prio: Option<u64>,
        redirect: Option<Vec<u8>>,
        nav_stop: Option<bool>,
        nav_label: Option<Vec<u8>>,
    },
    Headers {
        data: Vec<u8>,
    },
    SubPages {
        names: Vec<Vec<u8>>,
        nav_labels: Vec<Vec<u8>>,
        prios: Vec<u64>,
    },
    Macro {
        data: Vec<u8>,
    },
    String {
        data: Vec<u8>,
    },
}

impl MsgSerde<Msg> for Msg {
    fn msg_serialize(&self) -> ah::Result<Vec<u8>> {
        let mut payload = bincode_config().serialize(self)?;
        let mut ret = bincode_config().serialize(&MsgHdr {
            magic: MAGIC,
            payload_len: payload.len().try_into().unwrap(),
        })?;
        ret.append(&mut payload);
        Ok(ret)
    }

    fn try_msg_deserialize(buf: &[u8]) -> ah::Result<DeserializeResult<Msg>> {
        let hdr_len = MsgHdr::len();
        if buf.len() < hdr_len {
            Ok(DeserializeResult::Pending(hdr_len - buf.len()))
        } else {
            let hdr: MsgHdr = bincode_config()
                .deserialize(&buf[0..hdr_len])
                .context("Deserialize MsgHdr")?;
            if hdr.magic != MAGIC {
                return Err(ah::format_err!("Deserialize: Invalid MAGIC code."));
            }
            let full_len = hdr_len
                .checked_add(hdr.payload_len.try_into().unwrap())
                .context("Msg length overflow")?;
            if buf.len() < full_len {
                Ok(DeserializeResult::Pending(full_len - buf.len()))
            } else {
                let msg = bincode_config()
                    .deserialize(&buf[hdr_len..full_len])
                    .context("Deserialize Msg")?;
                Ok(DeserializeResult::Ok(msg))
            }
        }
    }
}

// vim: ts=4 sw=4 expandtab
